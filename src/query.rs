//! Turns a widget binding into a small result table: window the data to the
//! reporting period, filter, group, aggregate, sort and cut to top-N.

use crate::data::{self, LoadCtx, Table, Val};
use crate::model::{Binding, Dataset, Doc, Filter, Measure, Params, Widget};
use crate::period::{self, Range};
use crate::python::PyEngine;
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

#[derive(Default)]
struct Acc {
    sum: f64,
    count: u64,
    min: Option<f64>,
    max: Option<f64>,
    first: Option<f64>,
    last: Option<f64>,
    distinct: HashSet<String>,
    div: f64,
    /// Per-date sums (value, divisor) for balance-style measures.
    daily: BTreeMap<chrono::NaiveDate, (f64, f64)>,
    /// The first text value (for "text" measures such as an ISIN).
    text: Option<String>,
}

/// Aggregations for balances/snapshots, which don't add up over time.
pub fn semi_additive(agg: &str) -> bool {
    matches!(agg, "closing" | "opening" | "avg_daily")
}

impl Acc {
    fn push(&mut self, v: &Val, div: Option<&Val>, date: Option<chrono::NaiveDate>, track: bool) {
        let dv = div.and_then(|d| d.num());
        if let Some(d) = dv {
            self.div += d;
        }
        if track {
            if let Some(day) = date {
                let e = self.daily.entry(day).or_insert((0.0, 0.0));
                e.0 += v.num().unwrap_or(0.0);
                e.1 += dv.unwrap_or(0.0);
            }
        }
        if v.is_null() {
            return;
        }
        if self.text.is_none() {
            self.text = Some(v.text());
        }
        self.distinct.insert(v.text());
        self.count += 1;
        if let Some(n) = v.num() {
            self.sum += n;
            self.min = Some(self.min.map_or(n, |m| m.min(n)));
            self.max = Some(self.max.map_or(n, |m| m.max(n)));
            if self.first.is_none() {
                self.first = Some(n);
            }
            self.last = Some(n);
        }
    }

    fn value(&self, m: &Measure) -> Option<f64> {
        let ratio = m.divide_by.as_deref().is_some_and(|d| !d.is_empty());
        if semi_additive(&m.agg) && !self.daily.is_empty() {
            let pick = |(v, d): (f64, f64)| if ratio { if d != 0.0 { Some(v / d) } else { None } } else { Some(v) };
            return match m.agg.as_str() {
                "closing" => self.daily.values().next_back().copied().and_then(pick),
                "opening" => self.daily.values().next().copied().and_then(pick),
                _ => {
                    let n = self.daily.len() as f64;
                    let (v, d) = self.daily.values().fold((0.0, 0.0), |a, x| (a.0 + x.0, a.1 + x.1));
                    pick((v / n, d / n))
                }
            };
        }
        if ratio {
            return if self.div != 0.0 { Some(self.sum / self.div) } else { None };
        }
        match m.agg.as_str() {
            "count" => Some(self.count as f64),
            "distinct" => Some(self.distinct.len() as f64),
            "avg" => (self.count > 0).then(|| self.sum / self.count as f64),
            "min" => self.min,
            "max" => self.max,
            "first" => self.first,
            "last" => self.last,
            _ => (self.count > 0).then_some(self.sum),
        }
    }
}

/// Splits "date:month" into ("date", Some("month")).
fn dim_parts(d: &str) -> (&str, Option<&str>) {
    match d.rsplit_once(':') {
        Some((f, g)) if ["day", "week", "month", "quarter", "year"].contains(&g) => (f, Some(g)),
        _ => (d, None),
    }
}

fn resolve_value<'a>(v: &'a str, values: &'a BTreeMap<String, String>) -> Option<&'a str> {
    let t = v.trim();
    if let Some(name) = t.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        let pv = values.get(name.trim()).map(|s| s.as_str()).unwrap_or("");
        if pv.is_empty() || pv.eq_ignore_ascii_case("all") {
            return None;
        }
        return Some(pv);
    }
    Some(v)
}

fn matches(v: &Val, op: &str, want: &str) -> bool {
    let num = v.num().zip(want.trim().parse::<f64>().ok());
    let cmp = |o: std::cmp::Ordering| -> bool {
        match op {
            ">" => o.is_gt(),
            ">=" => o.is_ge(),
            "<" => o.is_lt(),
            "<=" => o.is_le(),
            _ => false,
        }
    };
    // a parameter with several values: any of them
    if want.contains('\u{1f}') {
        let any = crate::model::param_values(want).iter().any(|w| matches(v, if op == "!=" { "=" } else { op }, w));
        return if op == "!=" { !any } else { any };
    }
    match op {
        "!=" => !v.text().eq_ignore_ascii_case(want.trim()),
        "in" => want.split(',').any(|w| v.text().eq_ignore_ascii_case(w.trim())),
        "contains" => v.text().to_lowercase().contains(&want.to_lowercase()),
        ">" | ">=" | "<" | "<=" => match num {
            Some((a, b)) => cmp(a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)),
            None => match (v.date(), data::parse_date(want)) {
                (Some(a), Some(b)) => cmp(a.cmp(&b)),
                _ => cmp(v.text().as_str().cmp(want)),
            },
        },
        _ => match num {
            Some((a, b)) => a == b,
            None => v.text().eq_ignore_ascii_case(want.trim()),
        },
    }
}

/// SQL over the report's data with its values filled in: `:period_start`,
/// `:period_end` (the period; the older `:start`, `:end` too), `:prev_start`,
/// `:prev_end` (the comparison), `:data_start`, `:data_end` (the data loaded),
/// and `:name` for each parameter and variable.
pub fn bind_report(sql: &str, doc: &Doc) -> String {
    if !sql.contains(':') {
        return sql.to_string();
    }
    let p = &doc.params;
    let mut values = sql_values(doc);
    let data = Engine::needed_range(doc).unwrap_or_else(|| period::period(p));
    values.insert("data_start".into(), data.start.format("%Y-%m-%d").to_string());
    values.insert("data_end".into(), data.end.format("%Y-%m-%d").to_string());
    for d in &doc.param_defs {
        values.entry(d.name.clone()).or_insert_with(|| d.default.clone());
    }
    let (ps, pe) = match period::compare(p) {
        Some(c) => (c.start.format("%Y-%m-%d").to_string(), c.end.format("%Y-%m-%d").to_string()),
        None => ("0001-01-01".into(), "0001-01-01".into()),
    };
    values.insert("prev_start".into(), ps);
    values.insert("prev_end".into(), pe);
    data::bind_sql(sql, Some(period::period(p)), &values)
}

/// Rows that match every parameter set to filter this dataset (by its column).
pub fn param_filtered(doc: &Doc, ds: &Dataset, t: Arc<Table>) -> Arc<Table> {
    let mut active = Vec::new();
    for d in &doc.param_defs {
        let Some(name) = d.column_in(&ds.id) else { continue };
        let Some(c) = t.col(name) else { continue };
        let v = doc.params.values.get(&d.name).cloned().unwrap_or_else(|| d.default.clone());
        if crate::model::param_values(&v).is_empty() {
            continue;
        }
        active.push((c, v));
    }
    if active.is_empty() {
        return t;
    }
    Arc::new(Table {
        columns: t.columns.clone(),
        rows: t.rows.iter().filter(|r| active.iter().all(|(c, v)| matches(r.get(*c).unwrap_or(&Val::Null), "=", &format!("{v}\u{1f}")))).cloned().collect(),
    })
}

fn apply_filters(t: &Table, filters: &[Filter], values: &BTreeMap<String, String>) -> Result<Table, String> {
    let mut active = Vec::new();
    for f in filters {
        if f.field.is_empty() {
            continue;
        }
        let Some(v) = resolve_value(&f.value, values) else { continue };
        let c = t.col(&f.field).ok_or_else(|| format!("Unknown field '{}'", f.field))?;
        active.push((c, f.op.as_str(), v.to_string()));
    }
    if active.is_empty() {
        return Ok(t.clone());
    }
    Ok(Table {
        columns: t.columns.clone(),
        rows: t
            .rows
            .iter()
            .filter(|r| active.iter().all(|(c, op, v)| matches(r.get(*c).unwrap_or(&Val::Null), op, v)))
            .cloned()
            .collect(),
    })
}

struct Grouped {
    /// (sort keys, labels, accumulators per measure)
    groups: Vec<(Vec<String>, Vec<String>, Vec<Acc>)>,
}

fn group(t: &Table, dims: &[String], measures: &[Measure], dc: Option<usize>) -> Result<Grouped, String> {
    let mut dcols = Vec::new();
    for d in dims {
        let (f, g) = dim_parts(d);
        let c = t.col(f).ok_or_else(|| format!("Unknown field '{f}'"))?;
        dcols.push((c, g));
    }
    let mut mcols = Vec::new();
    for m in measures {
        let c = if m.agg == "count" && (m.field.is_empty() || m.field == "*") {
            None
        } else {
            Some(t.col(&m.field).ok_or_else(|| format!("Unknown field '{}'", m.field))?)
        };
        let d = match m.divide_by.as_deref() {
            Some(d) if !d.is_empty() => Some(t.col(d).ok_or_else(|| format!("Unknown field '{d}'"))?),
            _ => None,
        };
        mcols.push((c, d));
    }
    let mut index: HashMap<Vec<String>, usize> = HashMap::new();
    let mut groups: Vec<(Vec<String>, Vec<String>, Vec<Acc>)> = Vec::new();
    let one = Val::Num(1.0);
    let track: Vec<bool> = measures.iter().map(|m| semi_additive(&m.agg) && dc.is_some()).collect();
    let any_track = track.iter().any(|x| *x);
    for row in &t.rows {
        let day = if any_track { dc.and_then(|c| row.get(c)).and_then(|v| v.date()) } else { None };
        let mut keys = Vec::with_capacity(dcols.len());
        let mut labels = Vec::with_capacity(dcols.len());
        for (c, g) in &dcols {
            let v = row.get(*c).unwrap_or(&Val::Null);
            match (g, v.date()) {
                (Some(g), Some(d)) => {
                    let (k, l) = period::bucket(d, g);
                    keys.push(k);
                    labels.push(l);
                }
                _ => {
                    let s = if v.is_null() { "(blank)".to_string() } else { v.text() };
                    let k = match v {
                        Val::Num(n) => format!("{:020.6}", n + 1e12),
                        _ => s.to_lowercase(),
                    };
                    keys.push(k);
                    labels.push(s);
                }
            }
        }
        let gi = *index.entry(keys.clone()).or_insert_with(|| {
            groups.push((keys, labels, measures.iter().map(|_| Acc::default()).collect()));
            groups.len() - 1
        });
        for (mi, (c, d)) in mcols.iter().enumerate() {
            let v = c.map(|c| row.get(c).unwrap_or(&Val::Null)).unwrap_or(&one);
            groups[gi].2[mi].push(v, d.and_then(|d| row.get(d)), day, track[mi]);
        }
    }
    Ok(Grouped { groups })
}

fn total(t: &Table, measures: &[Measure], dc: Option<usize>) -> Result<Vec<Option<f64>>, String> {
    let g = group(t, &[], measures, dc)?;
    Ok(match g.groups.first() {
        Some((_, _, accs)) => measures.iter().zip(accs).map(|(m, a)| a.value(m)).collect(),
        None => measures.iter().map(|m| if m.agg == "count" || m.agg == "distinct" { Some(0.0) } else { None }).collect(),
    })
}

fn opt_json(v: Option<f64>) -> Json {
    match v {
        Some(n) if n.is_finite() => json!(n),
        _ => Json::Null,
    }
}

/// Applies a measure's calc ("prev", "delta", "pct") given current and comparison values.
fn calc(m: &Measure, cur: Option<f64>, prev: Option<f64>) -> Option<f64> {
    match m.calc.as_str() {
        "prev" => prev,
        "delta" => cur.zip(prev).map(|(c, p)| c - p),
        "pct" => cur.zip(prev).and_then(|(c, p)| if p != 0.0 { Some((c - p) / p.abs()) } else { None }),
        _ => cur,
    }
}

fn needs_compare(measures: &[Measure]) -> bool {
    measures.iter().any(|m| matches!(m.calc.as_str(), "prev" | "delta" | "pct"))
}

/// Orders JSON cells: numbers numerically, everything else as text.
fn cmp_json(a: &Json, b: &Json) -> std::cmp::Ordering {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => {
            let (x, y) = (a.as_str().unwrap_or(""), b.as_str().unwrap_or(""));
            x.to_lowercase().cmp(&y.to_lowercase())
        }
    }
}

/// "3:desc" → sort by output column 3, descending.
fn column_sort(sort: &str) -> Option<(usize, bool)> {
    let (i, dir) = sort.split_once(':')?;
    Some((i.parse().ok()?, dir == "desc"))
}

fn dim_key(v: &Val, grain: Option<&str>) -> String {
    match (grain, v.date()) {
        (Some(g), Some(d)) => period::bucket(d, g).0,
        _ => match v {
            Val::Num(n) => format!("{:020.6}", n + 1e12),
            Val::Null => "(blank)".into(),
            _ => v.text().to_lowercase(),
        },
    }
}

/// Explains an empty result: wrong period, filters, unreadable dates.
fn diagnose(table: &Table, base: &Table, date_col: Option<usize>, win: Option<Range>, params: &Params) -> Json {
    if table.rows.is_empty() {
        return json!({"reason": "empty", "message": "The data source returned no rows."});
    }
    if base.rows.is_empty() {
        return json!({"reason": "filters", "message": "No rows match this widget's filters."});
    }
    if let (Some(c), Some(w)) = (date_col, win) {
        let dates: Vec<chrono::NaiveDate> = base.rows.iter().filter_map(|r| r.get(c).and_then(|v| v.date())).collect();
        let field = &table.columns[c];
        if dates.is_empty() {
            return json!({"reason": "dates", "message": format!("The date field “{field}” has no dates Quire can read. Pick another date field or set Time to “All data”.")});
        }
        let (lo, hi) = (*dates.iter().min().unwrap(), *dates.iter().max().unwrap());
        if !dates.iter().any(|d| w.contains(*d)) {
            let fmt = |d: chrono::NaiveDate| d.format("%-d %b %Y").to_string();
            return json!({
                "reason": "period",
                "message": format!("No rows in {}. The data runs from {} to {}.", period::label(params), fmt(lo), fmt(hi)),
                "latest": hi.format("%Y-%m-%d").to_string(),
            });
        }
    }
    json!({"reason": "none", "message": "No data for this selection."})
}

/// Everything a widget needs to render its data.
pub fn run(w: &Widget, b: &Binding, ds: &Dataset, table: &Table, params: &Params) -> Result<Json, String> {
    let detail = b.mode == "rows";
    if !detail && b.measures.is_empty() && b.dims.is_empty() {
        return Err("Pick at least one field to show.".into());
    }
    if detail && b.columns.is_empty() {
        return Err("Pick the columns to list.".into());
    }
    let date_col = ds.date_field.as_deref().filter(|s| !s.is_empty()).and_then(|f| table.col(f));
    let win = period::window(params, &b.window);
    let base = apply_filters(table, &b.filters, &params.values)?;
    let cur = match (date_col, win) {
        (Some(c), Some(r)) => base.filter_dates(c, &r),
        _ => base.clone(),
    };
    let cmp_table = match (date_col, period::window_compare(params, &b.window)) {
        (Some(c), Some(r)) => Some(base.filter_dates(c, &r)),
        _ => None,
    };

    if detail {
        return Ok(run_rows(b, table, &cur, &base, date_col, win, params));
    }

    let measures = &b.measures;
    let nd = b.dims.len();
    let mut out = json!({
        "columns": measures.iter().enumerate().map(|(i, m)| json!({
            "key": if m.id.is_empty() { format!("m{i}") } else { m.id.clone() }, "field": m.field,
            "label": if m.label.is_empty() { default_label(m) } else { m.label.clone() },
            "role": "measure", "format": m.format, "agg": m.agg,
        })).collect::<Vec<_>>(),
        "window": win,
    });

    if w.kind == "kpi" {
        // a value per measure: numbers, or the text of a "text" measure
        let cells = |accs: &[Acc]| -> Vec<Json> {
            measures.iter().zip(accs).map(|(m, a)| if m.agg == "text" { a.text.clone().map(Json::String).unwrap_or(Json::Null) } else { opt_json(a.value(m)) }).collect()
        };
        if nd > 0 {
            // with a "Group by": the categories in order (the top one first),
            // each with its values and its share of the first measure
            let mut cats: Vec<(String, Vec<Json>)> = group(&cur, &b.dims, measures, date_col)?.groups.iter().map(|(_, labels, accs)| (labels.join(" · "), cells(accs))).collect();
            // categories without a value (e.g. cash has no return) are left out
            cats.retain(|c| c.1.first().is_some_and(|v| !v.is_null()));
            let num = |c: &(String, Vec<Json>)| c.1.first().and_then(|v| v.as_f64()).unwrap_or(f64::NEG_INFINITY);
            match b.sort.as_str() {
                "value_asc" => cats.sort_by(|a, z| num(a).total_cmp(&num(z))),
                "dim_asc" => cats.sort_by(|a, z| a.0.cmp(&z.0)),
                "dim_desc" => cats.sort_by(|a, z| z.0.cmp(&a.0)),
                _ => cats.sort_by(|a, z| num(z).total_cmp(&num(a))),
            }
            let all: f64 = cats.iter().map(|c| c.1.first().and_then(|v| v.as_f64()).unwrap_or(0.0)).sum();
            let top = cats.first();
            out["kpi"] = json!({
                "value": top.and_then(|c| c.1.first().cloned()).unwrap_or(Json::Null),
                "values": top.map(|c| c.1.clone()).unwrap_or_default(),
                "name": top.map(|c| c.0.clone()),
                "cats": cats.iter().map(|(n, v)| json!({"name": n, "values": v, "share": if all != 0.0 { v.first().and_then(|x| x.as_f64()).map(|x| x / all) } else { None }})).collect::<Vec<_>>(),
                "spark": [],
            });
            if cur.rows.is_empty() {
                out["empty"] = diagnose(table, &base, date_col, win, params);
            }
            return Ok(out);
        }
        let texts = measures.iter().any(|m| m.agg == "text");
        let value = total(&cur, measures, date_col)?;
        let prev = match &cmp_table {
            Some(t) => Some(total(t, measures, date_col)?),
            None => None,
        };
        // monthly sparkline over the trailing year
        let spark = match (date_col, period::window(params, "trailing12")) {
            (Some(c), Some(r)) if !measures.is_empty() => {
                let t = base.filter_dates(c, &r);
                let dname = format!("{}:month", table.columns[c]);
                let g = group(&t, &[dname], &measures[..1], date_col)?.groups;
                let mut by: HashMap<String, Option<f64>> =
                    g.iter().map(|(k, _, a)| (k[0].clone(), a.first().and_then(|a| a.value(&measures[0])))).collect();
                let mut pts = Vec::new();
                let mut d = r.start;
                while d <= r.end {
                    let (k, l) = period::bucket(d, "month");
                    pts.push(json!({"k": l, "v": opt_json(by.remove(&k).flatten())}));
                    d = period::add_months(d, 1);
                }
                pts
            }
            _ => Vec::new(),
        };
        let v0 = value.first().copied().flatten();
        let p0 = prev.as_ref().and_then(|p| p.first().copied().flatten());
        let delta = v0.zip(p0).map(|(v, p)| v - p);
        let pct = v0.zip(p0).and_then(|(v, p)| if p != 0.0 { Some((v - p) / p.abs()) } else { None });
        let vals: Vec<Json> = if texts { group(&cur, &[], measures, date_col)?.groups.first().map(|g| cells(&g.2)).unwrap_or_default() } else { value.iter().map(|v| opt_json(*v)).collect() };
        out["kpi"] = json!({
            "value": vals.first().cloned().unwrap_or_else(|| opt_json(v0)),
            "values": vals,
            "prev": opt_json(p0),
            "delta": opt_json(delta),
            "pct": opt_json(pct),
            "spark": spark,
        });
        if cur.rows.is_empty() {
            out["empty"] = diagnose(table, &base, date_col, win, params);
        }
        return Ok(out);
    }

    let groups_total;
    let is_table = w.kind == "table";
    let topt = |k: &str| w.opts.pointer(&format!("/table/{k}")).and_then(|v| v.as_bool());
    let pivot_dim = b.pivot.trim().to_string();
    let pivot = if is_table && !pivot_dim.is_empty() {
        let g = group(&cur, std::slice::from_ref(&pivot_dim), measures, date_col)?.groups;
        let mut keys: Vec<(String, String)> = g.into_iter().map(|(k, l, _)| (k[0].clone(), l[0].clone())).collect();
        keys.sort();
        keys.truncate(PIVOT_CAP);
        Some((pivot_dim.clone(), keys))
    } else {
        None
    };
    let row_totals = topt("rowTotals").unwrap_or(true);
    let nm = measures.len();
    let mid = |i: usize, m: &Measure| if m.id.is_empty() { format!("m{i}") } else { m.id.clone() };
    let trend = is_table && pivot.is_none() && measures.iter().any(|m| m.calc == "trend");
    let trail = match (trend, date_col, period::window(params, "trailing12")) {
        (true, Some(c), Some(r)) => {
            let t = base.filter_dates(c, &r);
            let mut months = Vec::new();
            let mut d = r.start;
            while d <= r.end {
                months.push(period::bucket(d, "month").0);
                d = period::add_months(d, 1);
            }
            Some((t, format!("{}:month", table.columns[c]), months))
        }
        _ => None,
    };
    let ctx = Cells {
        measures,
        dc: date_col,
        pivot: pivot.as_ref(),
        row_totals,
        trail: trail.as_ref(),
        is_table,
    };
    let cmp_used = cmp_table.as_ref().filter(|_| needs_compare(measures));

    // ---- columns
    let mut columns: Vec<Json> = b
        .dims
        .iter()
        .map(|d| {
            let (f, g) = dim_parts(d);
            json!({"key": d, "label": pretty(f), "role": "dim", "grain": g})
        })
        .collect();
    let mlabel = |m: &Measure| if m.label.is_empty() { default_label(m) } else { m.label.clone() };
    match &pivot {
        Some((_, pkeys)) => {
            for (pk, pl) in pkeys {
                for (i, m) in measures.iter().enumerate() {
                    columns.push(json!({
                        "key": format!("{}@{pk}", mid(i, m)), "base": mid(i, m), "field": m.field,
                        "label": if nm == 1 { pl.clone() } else { mlabel(m) },
                        "group": if nm == 1 { Json::Null } else { json!(pl) },
                        "role": "measure", "format": m.format, "agg": m.agg,
                    }));
                }
            }
            if row_totals {
                for (i, m) in measures.iter().enumerate() {
                    columns.push(json!({
                        "key": format!("{}@total", mid(i, m)), "base": mid(i, m), "field": m.field,
                        "label": if nm == 1 { period::tr("Total") } else { mlabel(m) },
                        "group": if nm == 1 { Json::Null } else { json!(period::tr("Total")) },
                        "role": "measure", "format": m.format, "agg": m.agg, "row_total": true,
                    }));
                }
            }
        }
        None => {
            for (i, m) in measures.iter().enumerate() {
                let spark = ctx.trail.is_some() && m.calc == "trend";
                columns.push(json!({
                    "key": mid(i, m), "base": mid(i, m), "field": m.field, "label": mlabel(m),
                    "role": "measure", "format": if spark { "sparkline".to_string() } else if m.calc == "share" { "percent".to_string() } else if m.calc == "rank" { "number".to_string() } else { m.format.clone() },
                    "agg": m.agg, "calc": m.calc,
                }));
            }
        }
    }
    let mut out = json!({ "columns": columns, "window": win });

    // ---- rows
    let pad = |labels: &[String]| -> Vec<Json> { (0..nd).map(|i| json!(labels.get(i).cloned().unwrap_or_default())).collect() };
    let to_row = |(k, l, c): (Vec<String>, Vec<String>, Vec<Json>)| -> (Vec<String>, Vec<Json>) {
        let mut r = pad(&l);
        r.extend(c);
        (k, r)
    };
    let mut rows: Vec<(Vec<String>, Vec<Json>)> = ctx.cells(&cur, cmp_used, true, &b.dims)?.into_iter().map(to_row).collect();

    let first_is_date = b.dims.first().map(|d| dim_parts(d).1.is_some()).unwrap_or(false);
    let sort = if b.sort.is_empty() {
        if first_is_date || b.dims.is_empty() { "dim_asc".to_string() } else { "value_desc".to_string() }
    } else {
        b.sort.clone()
    };
    // the value a "largest first" sort uses: the first measure (its row total when pivoted)
    let vcol = match &pivot {
        Some((_, pk)) if row_totals => nd + pk.len() * nm,
        _ => nd,
    };
    let sort_rows = |rows: &mut Vec<(Vec<String>, Vec<Json>)>| {
        let val = |r: &Vec<Json>| r.get(vcol).cloned().unwrap_or(Json::Null);
        match (sort.as_str(), column_sort(&sort)) {
            (_, Some((i, desc))) => rows.sort_by(|a, b| {
                let o = cmp_json(a.1.get(i).unwrap_or(&Json::Null), b.1.get(i).unwrap_or(&Json::Null));
                if desc { o.reverse() } else { o }
            }),
            ("value_desc", _) => rows.sort_by(|a, b| cmp_json(&val(&b.1), &val(&a.1)).then(a.0.cmp(&b.0))),
            ("value_asc", _) => rows.sort_by(|a, b| cmp_json(&val(&a.1), &val(&b.1)).then(a.0.cmp(&b.0))),
            ("dim_desc", _) => rows.sort_by(|a, b| b.0.cmp(&a.0)),
            ("data", _) => {}
            _ => rows.sort_by(|a, b| a.0.cmp(&b.0)),
        }
    };
    sort_rows(&mut rows);
    groups_total = rows.len();

    let hier = is_table && nd >= 2 && (topt("subtotals").unwrap_or(false) || topt("collapse").unwrap_or(false));
    let collapse = topt("collapse").unwrap_or(false);
    let mut meta: Vec<Json> = Vec::new();
    let mut other_row: Option<Vec<Json>> = None;
    if hier {
        let mut levels: Vec<Vec<(Vec<String>, Vec<Json>)>> = Vec::new();
        for k in 1..nd {
            let mut l: Vec<(Vec<String>, Vec<Json>)> = ctx.cells(&cur, cmp_used, true, &b.dims[..k])?.into_iter().map(to_row).collect();
            sort_rows(&mut l);
            levels.push(l);
        }
        if b.limit > 0 {
            levels[0].truncate(b.limit as usize);
        }
        let mut flat: Vec<(Vec<String>, Vec<Json>)> = Vec::new();
        fn emit(
            parent: &[String],
            depth: usize,
            nd: usize,
            collapse: bool,
            levels: &[Vec<(Vec<String>, Vec<Json>)>],
            leaves: &[(Vec<String>, Vec<Json>)],
            flat: &mut Vec<(Vec<String>, Vec<Json>)>,
            meta: &mut Vec<Json>,
        ) {
            if depth == nd - 1 {
                for r in leaves.iter().filter(|r| r.0[..depth] == *parent) {
                    flat.push(r.clone());
                    meta.push(json!({"level": depth, "kind": "row"}));
                }
                return;
            }
            for node in levels[depth].iter().filter(|r| r.0[..depth] == *parent) {
                flat.push(node.clone());
                meta.push(json!({"level": depth, "kind": "group"}));
                if !collapse {
                    emit(&node.0, depth + 1, nd, collapse, levels, leaves, flat, meta);
                }
            }
        }
        emit(&[], 0, nd, collapse, &levels, &rows, &mut flat, &mut meta);
        rows = flat;
    } else if b.limit > 0 && nd == 1 && rows.len() > b.limit as usize {
        let rest = rows.split_off(b.limit as usize);
        if b.others && !measures.is_empty() {
            // re-aggregate the tail from rows so averages, ratios and changes stay correct
            let keep: HashSet<String> = rows.iter().map(|x| x.0[0].clone()).collect();
            let (f, gr) = dim_parts(&b.dims[0]);
            let tail = |t: &Table| -> Table {
                let c = t.col(f).unwrap_or(0);
                Table {
                    columns: t.columns.clone(),
                    rows: t.rows.iter().filter(|r| !keep.contains(&dim_key(r.get(c).unwrap_or(&Val::Null), gr))).cloned().collect(),
                }
            };
            let cmp_tail = cmp_used.map(tail);
            let cells = ctx.cells(&tail(&cur), cmp_tail.as_ref(), false, &[])?;
            let mut row = vec![json!(format!("{} ({})", period::tr("Other"), rest.len()))];
            row.extend(cells.into_iter().next().map(|c| c.2).unwrap_or_default());
            other_row = Some(row);
        }
    } else if b.limit > 0 && nd >= 2 {
        // keep the top-N categories of the first dimension across all series
        let mut sums: HashMap<String, f64> = HashMap::new();
        for (k, r) in &rows {
            *sums.entry(k[0].clone()).or_default() += r.get(vcol).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
        }
        let mut ranked: Vec<(String, f64)> = sums.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let keep: HashSet<String> = ranked.into_iter().take(b.limit as usize).map(|x| x.0).collect();
        rows.retain(|x| keep.contains(&x.0[0]));
    }

    // a date first dim: each row's sortable key ("2026-03", "2026-Q1"…), so a chart can write it its own way
    let row_keys: Option<Vec<String>> = first_is_date.then(|| rows.iter().map(|(k, _)| k.first().cloned().unwrap_or_default()).collect());
    let mut out_rows: Vec<Vec<Json>> = rows.into_iter().map(|(_, r)| r).collect();
    let grand = ctx.cells(&cur, cmp_used, true, &[])?.into_iter().next().map(|c| c.2).unwrap_or_else(|| vec![Json::Null; nm]);
    let mut total_row: Vec<Json> = vec![Json::Null; nd];
    total_row.extend(grand.iter().cloned());

    // ---- share of total, rank and running totals (in display order)
    if pivot.is_none() {
        for (i, m) in measures.iter().enumerate() {
            let c = nd + i;
            let leaf = |k: usize| meta.get(k).map(|x| x["kind"] == "row").unwrap_or(true);
            match m.calc.as_str() {
                "share" => {
                    let t = total_row[c].as_f64().unwrap_or(0.0);
                    for r in out_rows.iter_mut() {
                        r[c] = match r[c].as_f64() {
                            Some(v) if t != 0.0 => json!(v / t),
                            _ => Json::Null,
                        };
                    }
                    total_row[c] = if t != 0.0 { json!(1.0) } else { Json::Null };
                }
                "running" => {
                    let mut acc = 0.0;
                    for (k, r) in out_rows.iter_mut().enumerate() {
                        if !leaf(k) {
                            r[c] = Json::Null;
                            continue;
                        }
                        acc += r[c].as_f64().unwrap_or(0.0);
                        r[c] = json!(acc);
                    }
                }
                "rank" => {
                    let mut vals: Vec<(usize, f64)> =
                        out_rows.iter().enumerate().filter(|(k, _)| leaf(*k)).filter_map(|(k, r)| r[c].as_f64().map(|v| (k, v))).collect();
                    // a list sorted lowest first (the biggest losses) ranks lowest first too
                    let asc = sort == "value_asc";
                    vals.sort_by(|a, b| if asc { a.1.total_cmp(&b.1) } else { b.1.total_cmp(&a.1) });
                    for r in out_rows.iter_mut() {
                        r[c] = Json::Null;
                    }
                    for (pos, (k, _)) in vals.into_iter().enumerate() {
                        out_rows[k][c] = json!(pos + 1);
                    }
                    total_row[c] = Json::Null;
                }
                _ => {}
            }
        }
    }

    // "Other" joins after ranking: it shares the total but takes no rank or running position
    if let Some(mut o) = other_row {
        if pivot.is_none() {
            for (i, m) in measures.iter().enumerate() {
                let c = nd + i;
                match m.calc.as_str() {
                    "share" => {
                        let grand_raw = grand.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0);
                        o[c] = match o[c].as_f64() {
                            Some(v) if grand_raw != 0.0 => json!(v / grand_raw),
                            _ => Json::Null,
                        };
                    }
                    "rank" | "running" => o[c] = Json::Null,
                    _ => {}
                }
            }
        }
        out_rows.push(o);
        if hier {
            meta.push(json!({"level": 0, "kind": "row"}));
        }
    }
    let totals: Vec<Json> = total_row[nd..].to_vec();
    if out_rows.is_empty() {
        out["empty"] = diagnose(table, &base, date_col, win, params);
    }
    out["rows"] = json!(out_rows);
    if let Some(mut keys) = row_keys {
        // "Other" (after the rows) has no date
        keys.resize(out_rows.len(), String::new());
        out["keys"] = json!(keys);
    }
    if hier {
        out["row_meta"] = Json::Array(meta);
    }
    out["groups"] = json!(groups_total);
    out["total"] = Json::Array(totals);
    out["total_row"] = Json::Array(total_row);
    Ok(out)
}

const PIVOT_CAP: usize = 36;

type CellRow = (Vec<String>, Vec<String>, Vec<Json>);

/// Builds row cells for any level of grouping: plain values with calcs,
/// pivoted cells with row totals, or per-row trend series.
struct Cells<'a> {
    measures: &'a [Measure],
    dc: Option<usize>,
    pivot: Option<&'a (String, Vec<(String, String)>)>,
    row_totals: bool,
    trail: Option<&'a (Table, String, Vec<String>)>,
    is_table: bool,
}

impl Cells<'_> {
    fn cells(&self, cur: &Table, cmp: Option<&Table>, with_trend: bool, prefix: &[String]) -> Result<Vec<CellRow>, String> {
        let ms = self.measures;
        let mut out: Vec<CellRow> = Vec::new();
        if let Some((pd, pkeys)) = self.pivot {
            let mut d2 = prefix.to_vec();
            d2.push(pd.clone());
            let mut by: HashMap<(Vec<String>, String), Vec<Option<f64>>> = HashMap::new();
            for (k, _, a) in group(cur, &d2, ms, self.dc)?.groups {
                let (rk, pk) = (k[..k.len() - 1].to_vec(), k[k.len() - 1].clone());
                by.insert((rk, pk), ms.iter().zip(&a).map(|(m, a)| a.value(m)).collect());
            }
            for (k, labels, accs) in group(cur, prefix, ms, self.dc)?.groups {
                let mut cells = Vec::new();
                for (pk, _) in pkeys {
                    let v = by.get(&(k.clone(), pk.clone()));
                    for i in 0..ms.len() {
                        cells.push(opt_json(v.and_then(|v| v[i])));
                    }
                }
                if self.row_totals {
                    for (i, m) in ms.iter().enumerate() {
                        cells.push(opt_json(accs[i].value(m)));
                    }
                }
                out.push((k, labels, cells));
            }
        } else {
            let cmp_map: HashMap<Vec<String>, Vec<Option<f64>>> = match cmp {
                Some(t) => group(t, prefix, ms, self.dc)?
                    .groups
                    .into_iter()
                    .map(|(k, _, a)| (k, ms.iter().zip(&a).map(|(m, a)| a.value(m)).collect()))
                    .collect(),
                None => HashMap::new(),
            };
            // trend series: the same grouping, split by month over the trailing year
            let mut trend: HashMap<Vec<String>, HashMap<String, Vec<Option<f64>>>> = HashMap::new();
            if let (true, Some((t, month_dim, _))) = (with_trend && self.is_table, self.trail) {
                let mut d2 = prefix.to_vec();
                d2.push(month_dim.clone());
                for (k, _, a) in group(t, &d2, ms, self.dc)?.groups {
                    let (rk, mk) = (k[..k.len() - 1].to_vec(), k[k.len() - 1].clone());
                    trend.entry(rk).or_default().insert(mk, ms.iter().zip(&a).map(|(m, a)| a.value(m)).collect());
                }
            }
            for (k, labels, accs) in group(cur, prefix, ms, self.dc)?.groups {
                let prev = cmp_map.get(&k);
                let cells = ms
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        if m.agg == "text" {
                            return accs[i].text.clone().map(Json::String).unwrap_or(Json::Null);
                        }
                        if m.calc == "trend" && self.is_table {
                            return match (self.trail, trend.get(&k)) {
                                (Some((_, _, months)), Some(series)) => {
                                    Json::Array(months.iter().map(|mk| opt_json(series.get(mk).and_then(|v| v[i]))).collect())
                                }
                                _ => Json::Null,
                            };
                        }
                        opt_json(calc(m, accs[i].value(m), prev.and_then(|p| p[i])))
                    })
                    .collect();
                out.push((k, labels, cells));
            }
        }
        if prefix.is_empty() && out.is_empty() {
            let n = match self.pivot {
                Some((_, pk)) => pk.len() * ms.len() + if self.row_totals { ms.len() } else { 0 },
                None => ms.len(),
            };
            out.push((Vec::new(), Vec::new(), vec![Json::Null; n]));
        }
        Ok(out)
    }
}

const DETAIL_CAP: usize = 2000;

/// Detail mode: the rows themselves, with the chosen columns.
fn run_rows(b: &Binding, table: &Table, cur: &Table, base: &Table, date_col: Option<usize>, win: Option<Range>, params: &Params) -> Json {
    let cols: Vec<(String, Option<usize>)> = b.columns.iter().map(|c| (c.clone(), table.col(c))).collect();
    let numeric: Vec<bool> = cols
        .iter()
        .map(|(_, i)| {
            i.map(|i| {
                let mut it = cur.rows.iter().map(|r| r.get(i).unwrap_or(&Val::Null)).filter(|v| !v.is_null()).peekable();
                it.peek().is_some() && it.all(|v| matches!(v, Val::Num(_)))
            })
            .unwrap_or(false)
        })
        .collect();
    let mut rows: Vec<Vec<Json>> = cur
        .rows
        .iter()
        .map(|r| cols.iter().map(|(_, i)| i.and_then(|i| r.get(i)).map(Val::to_json).unwrap_or(Json::Null)).collect())
        .collect();
    if let Some((i, desc)) = column_sort(&b.sort) {
        rows.sort_by(|a, b| {
            let o = cmp_json(a.get(i).unwrap_or(&Json::Null), b.get(i).unwrap_or(&Json::Null));
            if desc { o.reverse() } else { o }
        });
    }
    let total_rows = rows.len();
    let cap = if b.limit > 0 { b.limit as usize } else { DETAIL_CAP };
    rows.truncate(cap);
    let total_row: Vec<Json> = cols
        .iter()
        .zip(&numeric)
        .map(|((_, i), num)| match (i, num) {
            (Some(i), true) => json!(cur.rows.iter().filter_map(|r| r.get(*i).and_then(|v| v.num())).sum::<f64>()),
            _ => Json::Null,
        })
        .collect();
    let mut out = json!({
        "columns": cols.iter().zip(&numeric).map(|((c, i), num)| json!({
            "key": c, "label": pretty(c), "role": if *num { "measure" } else { "dim" },
            "format": if *num { "number" } else { "" }, "missing": i.is_none(),
        })).collect::<Vec<_>>(),
        "rows": rows,
        "rows_total": total_rows,
        "total_row": total_row,
        "window": win,
        "detail": true,
    });
    if total_rows == 0 {
        out["empty"] = diagnose(table, base, date_col, win, params);
    }
    out
}

/// "gross_profit" → "Gross profit"
fn pretty(f: &str) -> String {
    let s = f.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => s,
    }
}

fn default_label(m: &Measure) -> String {
    let f = if m.field.is_empty() || m.field == "*" { "Rows" } else { &m.field };
    match m.agg.as_str() {
        "count" if f == "Rows" => "Count".into(),
        "count" => format!("Count of {f}"),
        "distinct" => format!("Distinct {f}"),
        "avg" => format!("Avg {f}"),
        "min" => format!("Min {f}"),
        "max" => format!("Max {f}"),
        _ => pretty(f),
    }
}

// ------------------------------------------------------------------ engine

/// Loads datasets (cached per period for period-aware sources) and runs
/// every widget's query.
#[derive(Default)]
pub struct Engine {
    tables: HashMap<String, Result<Arc<Table>, String>>,
    results: HashMap<String, Json>,
    /// Sandboxed DuckDB for transforms, calculated fields and SQL filters.
    duck: Option<duckdb::Connection>,
    /// How deep Python sources reading other sources go (a loop stops at 3).
    py_depth: usize,
}

impl Engine {
    pub fn clear(&mut self) {
        self.tables.clear();
        self.results.clear();
    }

    fn duck(&mut self) -> Result<&duckdb::Connection, String> {
        if self.duck.is_none() {
            self.duck = Some(data::sandbox()?);
        }
        Ok(self.duck.as_ref().unwrap())
    }

    /// Rows of a table that meet a SQL condition.
    pub fn filtered(&mut self, ds: &Dataset, t: &Arc<Table>, cond: &str) -> Result<Arc<Table>, String> {
        let key = format!("where\u{0}{}\u{0}{}\u{0}{cond}", ds.id, Arc::as_ptr(t) as usize);
        if let Some(r) = self.tables.get(&key) {
            return r.clone();
        }
        let conn = self.duck()?;
        let r = data::duck_register(conn, "data", t)
            .and_then(|_| data::duck_query(conn, &format!("select * from data where ({cond})")))
            .map(|x| Arc::new(x.infer()))
            .map_err(|e| format!("SQL filter: {}", e.trim_start_matches("SQL error: ")));
        self.tables.insert(key, r.clone());
        r
    }

    /// Union of every window the document's widgets need, plus comparisons.
    pub fn needed_range(doc: &Doc) -> Option<Range> {
        let p = &doc.params;
        let mut r = period::period(p);
        if let Some(c) = period::compare(p) {
            r = r.union(&c);
        }
        for page in &doc.pages {
            for w in &page.widgets {
                if let Some(b) = &w.binding {
                    match period::window(p, &b.window) {
                        Some(x) => r = r.union(&x),
                        None => return None,
                    }
                    if let Some(x) = period::window_compare(p, &b.window) {
                        r = r.union(&x);
                    }
                    if w.kind == "kpi" {
                        if let Some(x) = period::window(p, "trailing12") {
                            r = r.union(&x);
                        }
                    }
                }
            }
        }
        Some(r)
    }

    /// A dataset's rows after loading, its SQL transform and calculated fields.
    /// A dataset's rows as the report sees them: the report's parameters filter
    /// every source that has their column, before anything else reads it.
    pub fn table(&mut self, doc: &Doc, ds: &Dataset, base: Option<&Path>, py: &mut PyEngine) -> Result<Arc<Table>, String> {
        let t = self.shaped(doc, ds, base, py, 0, true)?;
        Ok(self.by_params(doc, ds, t))
    }

    /// A dataset's rows before any parameter filters them (parameter choices, previews).
    pub fn table_all(&mut self, doc: &Doc, ds: &Dataset, base: Option<&Path>, py: &mut PyEngine) -> Result<Arc<Table>, String> {
        self.shaped(doc, ds, base, py, 0, false)
    }

    /// Rows that match the report's parameters, cached per table and values.
    fn by_params(&mut self, doc: &Doc, ds: &Dataset, t: Arc<Table>) -> Arc<Table> {
        if !doc.param_defs.iter().any(|d| d.column_in(&ds.id).is_some()) {
            return t;
        }
        let key = format!(
            "params\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
            ds.id,
            Arc::as_ptr(&t) as usize,
            serde_json::to_string(&doc.params.values).unwrap_or_default(),
            serde_json::to_string(&doc.param_defs).unwrap_or_default()
        );
        if let Some(Ok(r)) = self.tables.get(&key) {
            return r.clone();
        }
        let r = param_filtered(doc, ds, t);
        self.tables.insert(key, Ok(r.clone()));
        r
    }

    fn shaped(&mut self, doc: &Doc, ds: &Dataset, base: Option<&Path>, py: &mut PyEngine, depth: usize, filter: bool) -> Result<Arc<Table>, String> {
        let raw = self.raw_table(doc, ds, base, py)?;
        // parameters filter a source's own rows first, so sums, joins and windows see only them
        let raw = if filter { self.by_params(doc, ds, raw) } else { raw };
        // the report's period and parameters, as SQL literals
        // a SQL dataset is its query; other sources are their rows, shaped by a transform
        let bound = match &ds.source {
            crate::model::Source::Query { sql } => bind_report(sql.trim(), doc),
            _ => bind_report(ds.transform.trim(), doc),
        };
        let transform = bound.trim();
        let calcs: Vec<crate::model::CalcField> = ds
            .calc_fields
            .iter()
            .filter(|c| !c.name.trim().is_empty() && !c.expr.trim().is_empty())
            .map(|c| crate::model::CalcField { name: c.name.clone(), expr: bind_report(&c.expr, doc), ..c.clone() })
            .collect();
        if transform.is_empty() && calcs.is_empty() {
            return Ok(raw);
        }
        if depth > 3 {
            return Err(format!("“{}” refers to itself through other datasets.", ds.name));
        }
        // other datasets the transform mentions by name, for joins
        let lower = transform.to_lowercase();
        let mentions = |name: &str| {
            !name.is_empty()
                && lower.match_indices(name).any(|(i, _)| {
                    let before = lower[..i].chars().next_back();
                    let after = lower[i + name.len()..].chars().next();
                    !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
                })
        };
        let mut others = Vec::new();
        for o in doc.datasets.iter().filter(|o| o.id != ds.id) {
            let name = data::sql_name(&o.name);
            if name != "data" && mentions(&name) {
                let t = self.shaped(doc, o, base, py, depth + 1, filter)?;
                others.push((name, if filter { self.by_params(doc, o, t) } else { t }));
            }
        }
        let key = format!(
            "sql\u{0}{}\u{0}{}\u{0}{transform}\u{0}{}\u{0}{}",
            ds.id,
            Arc::as_ptr(&raw) as usize,
            serde_json::to_string(&calcs).unwrap_or_default(),
            others.iter().map(|(n, t)| format!("{n}:{}", Arc::as_ptr(t) as usize)).collect::<Vec<_>>().join(",")
        );
        if let Some(t) = self.tables.get(&key) {
            return t.clone();
        }
        let conn = self.duck()?;
        let run = || -> Result<Table, String> {
            for (n, t) in &others {
                data::duck_register(conn, n, t)?;
            }
            // a SQL dataset has no rows of its own
            if !raw.columns.is_empty() {
                data::duck_register(conn, "data", &raw)?;
            }
            let src = if transform.is_empty() { "select * from data".to_string() } else { transform.trim_end_matches(';').to_string() };
            let sql = if calcs.is_empty() {
                src
            } else {
                let exprs: Vec<String> = calcs.iter().map(|c| format!("({}) as {}", c.expr.trim().trim_end_matches(';'), data::quote_ident(c.name.trim()))).collect();
                format!("select *, {} from ({src}) as data", exprs.join(", "))
            };
            data::duck_query(conn, &sql).map(|t| t.infer())
        };
        let t = run().map(Arc::new).map_err(|e| format!("{} ({}).", e.trim_end_matches('.'), if transform.is_empty() { "calculated fields" } else { "SQL transform" }));
        if self.tables.len() > 64 {
            self.tables.clear();
        }
        self.tables.insert(key, t.clone());
        t
    }

    fn raw_table(&mut self, doc: &Doc, ds: &Dataset, base: Option<&Path>, py: &mut PyEngine) -> Result<Arc<Table>, String> {
        let aware = ds.source.period_aware();
        let range = if aware { Self::needed_range(doc) } else { None };
        let src = serde_json::to_string(&ds.source).unwrap_or_default();
        let key = format!(
            "{}\u{0}{src}\u{0}{:?}\u{0}{}",
            ds.id,
            range,
            if aware { serde_json::to_string(&sql_values(doc)).unwrap_or_default() } else { String::new() }
        );
        if let Some(t) = self.tables.get(&key) {
            return t.clone();
        }
        if self.tables.len() > 64 {
            self.tables.clear();
        }
        let values = sql_values(doc);
        // a Python script reads other sources by name: sources["Позиции"] (or their SQL name or id)
        let sources = match &ds.source {
            crate::model::Source::Python { code } if code.contains("sources[") && self.py_depth < 3 => {
                let mut m = serde_json::Map::new();
                for o in doc.datasets.iter().filter(|o| o.id != ds.id) {
                    let names = [o.name.clone(), data::sql_name(&o.name), o.id.clone()];
                    let named = names.iter().filter(|n| !n.is_empty()).find(|n| code.contains(&format!("sources[\"{n}\"]")) || code.contains(&format!("sources['{n}']")));
                    if let Some(n) = named {
                        self.py_depth += 1;
                        let t = self.table_all(doc, o, base, py);
                        self.py_depth -= 1;
                        let t = t.map_err(|e| format!("“{}” (read by this script): {e}", o.name))?;
                        m.insert(n.clone(), t.to_json());
                    }
                }
                Some(Json::Object(m))
            }
            _ => None,
        };
        let key = match &sources {
            Some(j) => {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                j.to_string().hash(&mut h);
                format!("{key}\u{0}{:x}", h.finish())
            }
            None => key,
        };
        if let Some(t) = self.tables.get(&key) {
            return t.clone();
        }
        let mut ctx = LoadCtx { base_dir: base, range, values: &values, py, sources };
        let t = crate::dbconn::resolve(doc, &ds.source).and_then(|src| data::load(&src, &mut ctx)).map(Arc::new);
        self.tables.insert(key, t.clone());
        t
    }

    /// Resolves `{{data:Source.field|agg}}`: the dataset's rows for the
    /// current parameters (a column named like a parameter follows it), up to
    /// the end of the period, then the last value (or first, sum, avg, min,
    /// max, count, list).
    pub fn lookup(&mut self, doc: &Doc, token: &str, base: Option<&Path>, py: &mut PyEngine) -> String {
        self.lookup_as(doc, token, base, py, false)
    }

    /// A data placeholder's value; `raw` keeps numbers at full precision (for variables that format it).
    pub fn lookup_as(&mut self, doc: &Doc, token: &str, base: Option<&Path>, py: &mut PyEngine, raw: bool) -> String {
        let num = |n: f64| if raw { n.to_string() } else { data::fmt_num_locale(n) };
        let (spec, agg) = token.split_once('|').map(|(a, b)| (a.trim(), b.trim().to_lowercase())).unwrap_or((token.trim(), "last".into()));
        let Some((dsn, field)) = spec.rsplit_once('.') else { return format!("[{token}?]") };
        let ds = doc.datasets.iter().find(|d| d.name.eq_ignore_ascii_case(dsn) || data::sql_name(&d.name) == dsn.to_lowercase() || d.id == dsn);
        let Some(ds) = ds else { return format!("[{dsn}?]") };
        let t = match self.table(doc, ds, base, py) {
            Ok(t) => t,
            Err(_) => return "—".into(),
        };
        let Some(c) = t.col(field) else { return format!("[{field}?]") };
        let end = period::period(&doc.params).end;
        let dcol = ds.date_field.as_deref().and_then(|f| t.col(f));
        let mut rows: Vec<&Vec<Val>> = t
            .rows
            .iter()
            .filter(|r| {
                doc.param_defs.iter().all(|pd| {
                    let v = doc.params.values.get(&pd.name).map(|s| s.as_str()).unwrap_or("");
                    if v.is_empty() || v.eq_ignore_ascii_case("all") {
                        return true;
                    }
                    match t.col(&pd.name) {
                        Some(pc) => r.get(pc).map(|x| x.text().eq_ignore_ascii_case(v)).unwrap_or(false),
                        None => true,
                    }
                })
            })
            .filter(|r| dcol.and_then(|dc| r.get(dc)).and_then(|v| v.date()).is_none_or(|d| d <= end))
            .collect();
        if let Some(dc) = dcol {
            rows.sort_by_key(|r| r.get(dc).and_then(|v| v.date()));
        }
        let vals: Vec<&Val> = rows.iter().filter_map(|r| r.get(c)).filter(|v| !v.is_null()).collect();
        let nums: Vec<f64> = vals.iter().filter_map(|v| v.num()).collect();
        let show = |v: &Val| match v {
            Val::Num(n) => num(*n),
            Val::Date(d) => period::fmt_date(*d),
            other => other.text(),
        };
        match agg.as_str() {
            "first" => vals.first().map(|v| show(v)).unwrap_or_default(),
            "sum" => num(nums.iter().sum()),
            "avg" if !nums.is_empty() => num(nums.iter().sum::<f64>() / nums.len() as f64),
            "min" => nums.iter().cloned().reduce(f64::min).map(num).unwrap_or_default(),
            "max" => nums.iter().cloned().reduce(f64::max).map(num).unwrap_or_default(),
            "count" => vals.len().to_string(),
            "distinct" => vals.iter().map(|v| v.text()).collect::<HashSet<_>>().len().to_string(),
            "list" => {
                let mut seen = Vec::new();
                for v in &vals {
                    let s = show(v);
                    if !seen.contains(&s) {
                        seen.push(s);
                    }
                }
                seen.join(", ")
            }
            _ => vals.last().map(|v| show(v)).unwrap_or_default(),
        }
    }

    /// Resolves `{{sql: …}}`: a query over the datasets, each available by
    /// its SQL name, with `:start`, `:end` and report parameters bound.
    /// The first cell of the first row is the result.
    pub fn sql_lookup(&mut self, doc: &Doc, query: &str, base: Option<&Path>, py: &mut PyEngine) -> String {
        self.sql_lookup_as(doc, query, base, py, false)
    }

    /// The first cell of a query; `raw` keeps numbers at full precision.
    pub fn sql_lookup_as(&mut self, doc: &Doc, query: &str, base: Option<&Path>, py: &mut PyEngine, raw: bool) -> String {
        let lower = query.to_lowercase();
        let mut tables = Vec::new();
        for ds in &doc.datasets {
            let name = data::sql_name(&ds.name);
            let mentioned = lower.match_indices(&name).any(|(i, _)| {
                let before = lower[..i].chars().next_back();
                let after = lower[i + name.len()..].chars().next();
                !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
            });
            if mentioned {
                match self.table(doc, ds, base, py) {
                    Ok(t) => tables.push((name, t)),
                    Err(e) => return format!("[{e}]"),
                }
            }
        }
        let sql = data::bind_sql(query, Some(period::period(&doc.params)), &doc.params.values);
        let conn = match self.duck() {
            Ok(c) => c,
            Err(e) => return format!("[{e}]"),
        };
        for (n, t) in &tables {
            if let Err(e) = data::duck_register(conn, n, t) {
                return format!("[{e}]");
            }
        }
        match data::duck_query(conn, &sql) {
            Ok(t) => match t.rows.first().and_then(|r| r.first()) {
                Some(Val::Num(n)) => if raw { n.to_string() } else { data::fmt_num_locale(*n) },
                Some(Val::Date(d)) => if raw { d.format("%Y-%m-%d").to_string() } else { period::fmt_date(*d) },
                Some(v) => v.text(),
                None => String::new(),
            },
            Err(e) => format!("[{}]", e.trim_start_matches("SQL error: ").lines().next().unwrap_or("SQL error")),
        }
    }

    /// Query results keyed by widget id.
    /// Choices of parameters that read them from a dataset column: its distinct values, sorted.
    pub fn param_options(&mut self, doc: &Doc, base: Option<&Path>, py: &mut PyEngine) -> Json {
        let mut out = serde_json::Map::new();
        for d in &doc.param_defs {
            let Some(src) = d.source.as_ref().filter(|s| !s.dataset.is_empty() && !s.field.is_empty()) else { continue };
            let Some(ds) = doc.dataset(&src.dataset) else { continue };
            let Ok(t) = self.table_all(doc, ds, base, py) else { continue };
            let Some(c) = t.col(&src.field) else { continue };
            let mut vals: Vec<String> = t.rows.iter().map(|r| r.get(c).map(|v| v.text()).unwrap_or_default()).filter(|s| !s.is_empty()).collect();
            vals.sort_by_key(|a| a.to_lowercase());
            vals.dedup();
            vals.truncate(500);
            out.insert(d.name.clone(), json!(vals));
        }
        Json::Object(out)
    }

    /// Every value a dataset's field takes, in the whole source (no period, filter or row limit): what a table
    /// column of it can show, for setting each value up (its pill, its icon). Sorted, at most 300.
    pub fn field_values(&mut self, doc: &Doc, dataset: &str, field: &str, base: Option<&Path>, py: &mut PyEngine) -> Vec<String> {
        let Some(ds) = doc.dataset(dataset) else { return vec![] };
        let Ok(t) = self.table_all(doc, ds, base, py) else { return vec![] };
        let Some(c) = t.col(field) else { return vec![] };
        let mut vals: Vec<String> = t.rows.iter().map(|r| r.get(c).map(|v| v.text()).unwrap_or_default()).filter(|s| !s.is_empty()).collect();
        vals.sort_by_key(|a| a.to_lowercase());
        vals.dedup();
        vals.truncate(300);
        vals
    }

    /// The text shown for each choice of parameters that read a label column: name → {value → label}.
    pub fn param_labels(&mut self, doc: &Doc, base: Option<&Path>, py: &mut PyEngine) -> Json {
        let mut out = serde_json::Map::new();
        for d in &doc.param_defs {
            let Some(src) = d.source.as_ref().filter(|s| !s.dataset.is_empty() && !s.field.is_empty() && !s.label.is_empty()) else { continue };
            let Some(ds) = doc.dataset(&src.dataset) else { continue };
            let Ok(t) = self.table_all(doc, ds, base, py) else { continue };
            let (Some(v), Some(l)) = (t.col(&src.field), t.col(&src.label)) else { continue };
            let mut m = serde_json::Map::new();
            for r in &t.rows {
                let key = r.get(v).map(|x| x.text()).unwrap_or_default();
                if !key.is_empty() {
                    m.entry(key).or_insert_with(|| json!(r.get(l).map(|x| x.text()).unwrap_or_default()));
                }
            }
            out.insert(d.name.clone(), Json::Object(m));
        }
        Json::Object(out)
    }

    pub fn results(&mut self, doc: &Doc, base: Option<&Path>, py: &mut PyEngine) -> HashMap<String, Json> {
        let mut out = HashMap::new();
        let pkey = format!("{}\u{0}{}\u{0}{}\u{0}{}", serde_json::to_string(&doc.params).unwrap_or_default(), doc.theme.locale, serde_json::to_string(&doc.param_defs).unwrap_or_default(), doc.access_src);
        if self.results.len() > 2000 {
            self.results.clear();
        }
        for page in &doc.pages {
            // a composite's parts get their data as widgets of their own ("parent::part")
            let all: Vec<std::borrow::Cow<Widget>> = page.widgets.iter().flat_map(|w| {
                std::iter::once(std::borrow::Cow::Borrowed(w)).chain(w.parts.iter().map(move |p| std::borrow::Cow::Owned(crate::model::part_widget(w, p))))
            }).collect();
            for w in &all {
                if w.kind == "composite" {
                    continue;
                }
                let w = &*crate::model::effective(&doc.theme, w);
                let Some(b) = &w.binding else { continue };
                // on a repeated page, filters may name the page's row: {{row.acct_cd}}
                let row_b;
                let b = match serde_json::to_string(b) {
                    Ok(js) if !page.row.is_empty() && js.contains("{{row.") => {
                        row_b = serde_json::from_str::<crate::model::Binding>(&crate::vm::fill_row(&js, &page.row, true)).unwrap_or_else(|_| b.clone());
                        &row_b
                    }
                    _ => b,
                };
                if b.dataset.is_empty() {
                    continue;
                }
                let Some(ds) = doc.dataset(&b.dataset) else {
                    out.insert(w.id.clone(), json!({"error": format!("Dataset '{}' not found", b.dataset)}));
                    continue;
                };
                let rkey = format!(
                    "{}\u{0}{}\u{0}{}\u{0}{pkey}\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
                    w.kind,
                    serde_json::to_string(b).unwrap_or_default(),
                    serde_json::to_string(&ds.source).unwrap_or_default(),
                    ds.date_field.as_deref().unwrap_or(""),
                    ds.transform,
                    serde_json::to_string(&ds.calc_fields).unwrap_or_default(),
                    serde_json::to_string(w.opts.get("table").unwrap_or(&Json::Null)).unwrap_or_default()
                );
                if let Some(r) = self.results.get(&rkey) {
                    out.insert(w.id.clone(), expand_labels(r.clone(), doc));
                    continue;
                }
                let cond = bind_report(b.where_sql.trim().trim_end_matches(';'), doc);
                let t = self.table(doc, ds, base, py).and_then(|t| if cond.is_empty() { Ok(t) } else { self.filtered(ds, &t, &cond) });
                let t = t.map(|t| param_filtered(doc, ds, t));
                let r = match t {
                    Ok(t) => run(w, b, ds, &t, &doc.params).unwrap_or_else(|e| json!({"error": e})),
                    Err(e) => json!({"error": e}),
                };
                self.results.insert(rkey, r.clone());
                out.insert(w.id.clone(), expand_labels(r, doc));
            }
        }
        out
    }
}

impl Engine {
    /// A data source's rows as text (column → value), for repeating pages.
    pub fn rows_text(&mut self, doc: &Doc, ds: &Dataset, base: Option<&Path>, py: &mut PyEngine) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
        let t = self.table(doc, ds, base, py)?;
        Ok(t.rows.iter().map(|r| t.columns.iter().zip(r).map(|(c, v)| (c.clone(), v.text())).collect()).collect())
    }
}

/// Column labels may hold placeholders ({{end}}, {{data:Source.field}}…), filled in for the current report.
fn expand_labels(mut r: Json, doc: &Doc) -> Json {
    if let Some(cols) = r.get_mut("columns").and_then(|c| c.as_array_mut()) {
        for c in cols {
            if let Some(l) = c.get("label").and_then(|l| l.as_str()).filter(|l| l.contains("{{")) {
                let e = crate::vm::expand(l, doc);
                c["label"] = Json::String(e);
            }
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::period::ymd;

    fn table() -> Table {
        let mut rows = Vec::new();
        for (m, region, rev) in [(1, "North", 10.0), (1, "South", 5.0), (2, "North", 20.0), (2, "South", 7.0), (3, "West", 1.0)] {
            rows.push(vec![Val::Date(ymd(2026, m, 10)), Val::Str(region.into()), Val::Num(rev)]);
        }
        Table { columns: vec!["date".into(), "region".into(), "revenue".into()], rows }
    }

    fn ds() -> Dataset {
        Dataset { id: "d1".into(), name: "sales".into(), date_field: Some("date".into()), ..Default::default() }
    }

    fn sum(field: &str) -> Measure {
        Measure { field: field.into(), agg: "sum".into(), ..Default::default() }
    }

    #[test]
    fn groups_and_sorts_by_value() {
        let p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let b = Binding { dataset: "d1".into(), dims: vec!["region".into()], measures: vec![sum("revenue")], ..Default::default() };
        let w = Widget { kind: "bar".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["rows"], json!([["North", 30.0], ["South", 12.0], ["West", 1.0]]));
        assert_eq!(r["total"], json!([43.0]));
    }

    #[test]
    fn top_n_with_other() {
        let p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let b = Binding {
            dims: vec!["region".into()],
            measures: vec![sum("revenue")],
            limit: 1,
            others: true,
            ..Default::default()
        };
        let w = Widget { kind: "table".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["rows"], json!([["North", 30.0], ["Other (2)", 13.0]]));
    }

    #[test]
    fn kpi_compares_with_previous_month() {
        let p = Params { grain: "month".into(), date: ymd(2026, 2, 1), compare: "previous".into(), ..Default::default() };
        let b = Binding { measures: vec![sum("revenue")], ..Default::default() };
        let w = Widget { kind: "kpi".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["kpi"]["value"], json!(27.0));
        assert_eq!(r["kpi"]["prev"], json!(15.0));
        assert_eq!(r["kpi"]["pct"], json!(0.8));
        assert_eq!(r["kpi"]["spark"].as_array().unwrap().len(), 12);
    }

    #[test]
    fn a_kpi_with_group_by_shows_the_top_category() {
        // "largest region": its name, values and share; the lowest first when sorted up
        let p = Params { grain: "month".into(), date: ymd(2026, 2, 1), ..Default::default() };
        let w = Widget { kind: "kpi".into(), ..Default::default() };
        let mut b = Binding { dims: vec!["region".into()], measures: vec![sum("revenue")], ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        let k = &r["kpi"];
        let cats = k["cats"].as_array().unwrap();
        assert_eq!(k["name"], cats[0]["name"]);
        assert_eq!(k["value"], cats[0]["values"][0]);
        let shares: f64 = cats.iter().map(|c| c["share"].as_f64().unwrap()).sum();
        assert!((shares - 1.0).abs() < 1e-9, "shares add up to 100%");
        assert!(cats.windows(2).all(|p| p[0]["values"][0].as_f64() >= p[1]["values"][0].as_f64()));
        b.sort = "value_asc".into();
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["kpi"]["name"], cats.last().unwrap()["name"]);
    }

    #[test]
    fn change_columns_and_column_sort() {
        let p = Params { grain: "month".into(), date: ymd(2026, 2, 1), compare: "previous".into(), ..Default::default() };
        let mut pct = sum("revenue");
        pct.calc = "pct".into();
        let mut delta = sum("revenue");
        delta.calc = "delta".into();
        let b = Binding { dims: vec!["region".into()], measures: vec![sum("revenue"), delta, pct], sort: "3:asc".into(), ..Default::default() };
        let w = Widget { kind: "table".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        // North 10 → 20 (+100%), South 5 → 7 (+40%); sorted by % change ascending
        assert_eq!(r["rows"], json!([["South", 7.0, 2.0, 0.4], ["North", 20.0, 10.0, 1.0]]));
        assert_eq!(r["total_row"], json!([null, 27.0, 12.0, 0.8]));
    }

    #[test]
    fn balances_use_closing_values_not_sums() {
        // two clients, daily balances over two months
        let mut rows = Vec::new();
        for (m, d, a, b) in [(1, 30, 100.0, 50.0), (1, 31, 110.0, 50.0), (2, 27, 120.0, 40.0), (2, 28, 130.0, 45.0)] {
            rows.push(vec![Val::Date(ymd(2026, m, d)), Val::Str("A".into()), Val::Num(a)]);
            rows.push(vec![Val::Date(ymd(2026, m, d)), Val::Str("B".into()), Val::Num(b)]);
        }
        let t = Table { columns: vec!["date".into(), "client".into(), "amount".into()], rows };
        let p = Params { grain: "month".into(), date: ymd(2026, 2, 1), compare: "previous".into(), ..Default::default() };
        let closing = Measure { field: "amount".into(), agg: "closing".into(), ..Default::default() };
        let w = Widget { kind: "kpi".into(), ..Default::default() };
        let b = Binding { measures: vec![closing.clone()], ..Default::default() };
        let r = run(&w, &b, &ds(), &t, &p).unwrap();
        assert_eq!(r["kpi"]["value"], json!(175.0), "Feb 28 balance, not the sum of days");
        assert_eq!(r["kpi"]["prev"], json!(160.0), "Jan 31 balance");
        let avg = Measure { field: "amount".into(), agg: "avg_daily".into(), ..Default::default() };
        let b = Binding { dims: vec!["client".into()], measures: vec![closing, avg], sort: "dim_asc".into(), ..Default::default() };
        let w = Widget { kind: "table".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &t, &p).unwrap();
        assert_eq!(r["rows"], json!([["A", 130.0, 125.0], ["B", 45.0, 42.5]]));
        assert_eq!(r["total_row"], json!([null, 175.0, 167.5]));
    }

    fn tbl() -> Widget {
        Widget { kind: "table".into(), ..Default::default() }
    }

    #[test]
    fn pivot_spreads_months_across_columns() {
        let p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let b = Binding { dims: vec!["region".into()], pivot: "date:month".into(), measures: vec![sum("revenue")], sort: "dim_asc".into(), ..Default::default() };
        let r = run(&tbl(), &b, &ds(), &table(), &p).unwrap();
        let labels: Vec<&str> = r["columns"].as_array().unwrap().iter().map(|c| c["label"].as_str().unwrap()).collect();
        assert_eq!(labels, vec!["Region", "Jan 26", "Feb 26", "Mar 26", "Total"]);
        assert_eq!(r["rows"], json!([["North", 10.0, 20.0, null, 30.0], ["South", 5.0, 7.0, null, 12.0], ["West", null, null, 1.0, 1.0]]));
        assert_eq!(r["total_row"], json!([null, 15.0, 27.0, 1.0, 43.0]));
    }

    #[test]
    fn subtotals_nest_groups() {
        let p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let mut w = tbl();
        w.opts = json!({"table": {"subtotals": true}});
        let b = Binding { dims: vec!["region".into(), "date:month".into()], measures: vec![sum("revenue")], sort: "dim_asc".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        let rows = r["rows"].as_array().unwrap();
        assert_eq!(rows[0], json!(["North", "", 30.0]));
        assert_eq!(rows[1], json!(["North", "Jan 26", 10.0]));
        assert_eq!(r["row_meta"][0], json!({"level": 0, "kind": "group"}));
        assert_eq!(r["row_meta"][1], json!({"level": 1, "kind": "row"}));
        w.opts = json!({"table": {"collapse": true}});
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["rows"].as_array().unwrap().len(), 3, "only the region totals");
    }

    #[test]
    fn share_rank_running_and_trend() {
        let p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let mk = |c: &str| Measure { calc: c.into(), ..sum("revenue") };
        let b = Binding { dims: vec!["region".into()], measures: vec![sum("revenue"), mk("share"), mk("rank"), mk("running"), mk("trend")], ..Default::default() };
        let other = Binding { limit: 1, others: true, ..b.clone() };
        let r2 = run(&tbl(), &other, &ds(), &table(), &p).unwrap();
        assert_eq!(r2["rows"][0][3], json!(1), "top row ranks first even with an Other row");
        assert_eq!(r2["rows"][1][3], Json::Null);
        assert!((r2["rows"][1][2].as_f64().unwrap() - 13.0 / 43.0).abs() < 1e-9);
        let r = run(&tbl(), &b, &ds(), &table(), &p).unwrap();
        let north = &r["rows"][0];
        assert_eq!(north[0], json!("North"));
        assert!((north[2].as_f64().unwrap() - 30.0 / 43.0).abs() < 1e-9);
        assert_eq!(north[3], json!(1));
        assert_eq!(r["rows"][1][4], json!(42.0), "running total");
        assert_eq!(north[5].as_array().unwrap().len(), 12, "12-month sparkline");
        assert_eq!(r["total_row"][2], json!(1.0));
    }

    #[test]
    fn detail_rows_list_records() {
        let p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let b = Binding { mode: "rows".into(), columns: vec!["region".into(), "revenue".into()], sort: "1:desc".into(), limit: 2, ..Default::default() };
        let w = Widget { kind: "table".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["rows"], json!([["North", 20.0], ["North", 10.0]]));
        assert_eq!(r["rows_total"], json!(5));
        assert_eq!(r["total_row"], json!([null, 43.0]));
        assert_eq!(r["columns"][1]["role"], json!("measure"));
    }

    #[test]
    fn empty_results_explain_the_period() {
        let p = Params { grain: "month".into(), date: ymd(2030, 1, 1), ..Default::default() };
        let b = Binding { dims: vec!["region".into()], measures: vec![sum("revenue")], ..Default::default() };
        let w = Widget { kind: "table".into(), ..Default::default() };
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["empty"]["reason"], json!("period"));
        assert_eq!(r["empty"]["latest"], json!("2026-03-10"));
    }

    #[test]
    fn param_filters_follow_report_values() {
        let mut p = Params { grain: "year".into(), date: ymd(2026, 6, 1), ..Default::default() };
        let b = Binding {
            dims: vec!["date:month".into()],
            measures: vec![sum("revenue")],
            filters: vec![Filter { field: "region".into(), op: "=".into(), value: "{{region}}".into() }],
            ..Default::default()
        };
        let w = Widget { kind: "line".into(), ..Default::default() };
        p.values.insert("region".into(), "North".into());
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["rows"], json!([["Jan 26", 10.0], ["Feb 26", 20.0]]));
        p.values.insert("region".into(), "All".into());
        let r = run(&w, &b, &ds(), &table(), &p).unwrap();
        assert_eq!(r["rows"].as_array().unwrap().len(), 3);
    }
}

/// The values of `:name` in SQL (besides `:start`/`:end`): the report's
/// parameters, the period itself (`:period_start`, `:period_end`) and the
/// designer's variables. Parameters win over variables of the same name.
pub fn sql_values(doc: &Doc) -> std::collections::BTreeMap<String, String> {
    let mut values = std::collections::BTreeMap::new();
    for (n, v, _) in crate::vm::sql_vars(doc) {
        values.insert(n, v);
    }
    if let Some(r) = crate::period::window(&doc.params, "period") {
        values.insert("period_start".into(), r.start.format("%Y-%m-%d").to_string());
        values.insert("period_end".into(), r.end.format("%Y-%m-%d").to_string());
    }
    // a parameter without a value yet has its default (or All): its :name always binds
    for d in &doc.param_defs {
        let v = if d.default.is_empty() { "All".to_string() } else { d.default.clone() };
        values.insert(d.name.clone(), v);
    }
    for (k, v) in &doc.params.values {
        values.insert(k.clone(), v.clone());
    }
    // the session's, whatever a document says
    values.insert("access_src".into(), doc.access_src.clone());
    values
}

/// Which data sources follow a report parameter, and which don't: a source
/// follows it through a column it filters (the parameter's "filters column"),
/// by naming it (`:name` in SQL, `params["name"]` in Python, `{{name}}`), or by
/// reading a source that follows it (SQL over sources, `sources["…"]`). The
/// source the parameter takes its choices from is apart: it lists them.
pub fn param_reach(doc: &Doc, name: &str) -> Json {
    let Some(def) = doc.param_defs.iter().find(|d| d.name == name) else { return json!(null) };
    let names = |code: &str| {
        let lower = code.to_lowercase();
        let n = name.to_lowercase();
        let word_at = |i: usize, len: usize| !lower[i + len..].starts_with(|c: char| c.is_alphanumeric() || c == '_');
        lower.match_indices(&format!(":{n}")).any(|(i, _)| (i == 0 || lower.as_bytes()[i - 1] != b':') && word_at(i, n.len() + 1))
            || lower.contains(&format!("params[\"{n}\"]")) || lower.contains(&format!("params['{n}']")) || lower.contains(&format!("{{{{{n}}}}}"))
    };
    let text_of = |ds: &Dataset| {
        let src = match &ds.source {
            crate::model::Source::Db { query, .. } => query.clone(),
            crate::model::Source::Query { sql } => sql.clone(),
            crate::model::Source::Python { code } => code.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        format!("{src}\n{}\n{}", ds.transform, ds.calc_fields.iter().map(|c| c.expr.as_str()).collect::<Vec<_>>().join("\n"))
    };
    let choices = def.source.as_ref().map(|s| s.dataset.clone()).unwrap_or_default();
    let mut follows: std::collections::HashSet<String> = doc
        .datasets
        .iter()
        .filter(|ds| def.column_in(&ds.id).is_some_and(|c| ds.fields.is_empty() || ds.fields.iter().any(|f| f.name.eq_ignore_ascii_case(c))) || names(&text_of(ds)))
        .map(|ds| ds.id.clone())
        .collect();
    // sources reading sources that follow it follow it too
    loop {
        let before = follows.len();
        for ds in &doc.datasets {
            if follows.contains(&ds.id) {
                continue;
            }
            let t = text_of(ds);
            let reads = doc.datasets.iter().filter(|o| follows.contains(&o.id)).any(|o| {
                t.contains(&format!("sources[\"{}\"]", o.name)) || t.contains(&format!("sources['{}']", o.name))
                    || (matches!(ds.source, crate::model::Source::Query { .. }) && t.to_lowercase().contains(&data::sql_name(&o.name)))
            });
            if reads {
                follows.insert(ds.id.clone());
            }
        }
        if follows.len() == before {
            break;
        }
    }
    let widgets = |id: &str| -> Vec<Json> {
        doc.pages.iter().enumerate().flat_map(|(pi, p)| p.widgets.iter().filter(|w| w.binding.as_ref().is_some_and(|b| b.dataset == id) && w.opts.get("mirror").is_none()).map(move |w| json!({"page": pi + 1, "title": w.title, "kind": w.kind}))).collect()
    };
    // "Not filtered" chosen for a source in the parameter's settings: left out on purpose
    let apart = |id: &str| def.columns.get(id).is_some_and(|c| c.trim().is_empty());
    let not: Vec<Json> = doc.datasets.iter().filter(|ds| !follows.contains(&ds.id) && ds.id != choices && !apart(&ds.id))
        .map(|ds| json!({"id": ds.id, "name": ds.name, "widgets": widgets(&ds.id)})).collect();
    let left: Vec<Json> = doc.datasets.iter().filter(|ds| !follows.contains(&ds.id) && apart(&ds.id)).map(|ds| json!(ds.name)).collect();
    json!({
        "name": name,
        "follow": follows.len(),
        "of": doc.datasets.iter().filter(|ds| ds.id != choices).count(),
        "choices": doc.dataset(&choices).map(|d| d.name.clone()),
        "not": not,
        "apart": left,
    })
}
