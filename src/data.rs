//! Tabular data: loading datasets from files, databases and Python into an
//! in-memory table the query engine works on.

use crate::model::{Field, Source};
use crate::period::Range;
use crate::python::PyEngine;
use chrono::{Duration, NaiveDate};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Num(f64),
    Str(String),
    Date(NaiveDate),
}

impl Val {
    pub fn num(&self) -> Option<f64> {
        match self {
            Val::Num(n) => Some(*n),
            Val::Str(s) => parse_num(s),
            _ => None,
        }
    }

    pub fn date(&self) -> Option<NaiveDate> {
        match self {
            Val::Date(d) => Some(*d),
            Val::Str(s) => parse_date(s),
            // Unix timestamps in seconds or milliseconds
            Val::Num(n) if (1e9..1e11).contains(n) => chrono::DateTime::from_timestamp(*n as i64, 0).map(|d| d.date_naive()),
            Val::Num(n) if (1e12..1e14).contains(n) => chrono::DateTime::from_timestamp_millis(*n as i64).map(|d| d.date_naive()),
            _ => None,
        }
    }

    pub fn text(&self) -> String {
        match self {
            Val::Null => String::new(),
            Val::Num(n) => fmt_num(*n),
            Val::Str(s) => s.clone(),
            Val::Date(d) => d.format("%Y-%m-%d").to_string(),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Val::Null) || matches!(self, Val::Str(s) if s.is_empty())
    }

    pub fn to_json(&self) -> Json {
        match self {
            Val::Null => Json::Null,
            Val::Num(n) if n.is_finite() => json!(n),
            Val::Num(_) => Json::Null,
            Val::Str(s) => json!(s),
            Val::Date(d) => json!(d.format("%Y-%m-%d").to_string()),
        }
    }

    pub fn from_json(v: &Json) -> Val {
        match v {
            Json::Null => Val::Null,
            Json::Bool(b) => Val::Num(if *b { 1.0 } else { 0.0 }),
            Json::Number(n) => Val::Num(n.as_f64().unwrap_or(0.0)),
            Json::String(s) => Val::Str(s.clone()),
            other => Val::Str(other.to_string()),
        }
    }
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Reads numbers as people write them: "1,234.5", "$1,200", "12,5", "1.234,56",
/// "(300)" for negatives and "45%" as 0.45.
pub fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() || t.len() > 40 {
        return None;
    }
    let paren = t.starts_with('(') && t.ends_with(')');
    let pct = t.ends_with('%');
    let mut t: String = t
        .trim_matches(|c| c == '(' || c == ')')
        .chars()
        .filter(|c| !matches!(c, '$' | '€' | '£' | '¥' | '₽' | '%' | ' ' | '\u{a0}' | '\'' | '+'))
        .collect();
    if !t.chars().any(|c| c.is_ascii_digit()) || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-' | 'e' | 'E')) {
        return None;
    }
    let (comma, dot) = (t.rfind(','), t.rfind('.'));
    let european = match (comma, dot) {
        (Some(c), Some(d)) => c > d,
        // a single comma followed by anything but 3 digits is a decimal comma
        (Some(c), None) => t.matches(',').count() == 1 && t.len() - c - 1 != 3,
        _ => false,
    };
    t = if european { t.replace('.', "").replace(',', ".") } else { t.replace(',', "") };
    let v: f64 = t.parse().ok()?;
    let v = if paren { -v.abs() } else { v };
    Some(if pct { v / 100.0 } else { v })
}

/// A number the way the report's language writes it (1 234 567,5 in Russian).
pub fn fmt_num_locale(n: f64) -> String {
    let loc = crate::period::locale();
    let (lang_group, lang_dec) = if loc.starts_with("ru") || loc.starts_with("fr") {
        ("\u{a0}", ",")
    } else if loc.starts_with("de") {
        (".", ",")
    } else {
        (",", ".")
    };
    let f = crate::period::formats();
    let group = match f.thousands.as_str() {
        "" => lang_group,
        "space" => "\u{a0}",
        "thin" => "\u{202f}",
        "'" => "’",
        "none" => "",
        other => other,
    }
    .to_string();
    let dec = if f.decimal.is_empty() { lang_dec.to_string() } else { f.decimal.clone() };
    let neg = n < 0.0;
    let a = n.abs();
    let whole = a.trunc() as u64;
    let frac = ((a - a.trunc()) * 100.0).round() as u64;
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push_str(&group);
        }
        grouped.push(ch);
    }
    let mut out = grouped;
    if frac > 0 && a < 1e6 {
        out.push_str(&dec);
        out.push_str(&format!("{frac:02}"));
    }
    match (neg, f.negative.as_str()) {
        (true, "paren") => format!("({out})"),
        (true, _) => format!("−{out}"),
        _ => out,
    }
}

const EXCEL_EPOCH: (i32, u32, u32) = (1899, 12, 30);

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.len() < 6 || s.len() > 30 {
        return None;
    }
    let head = s.split(['T', ' ']).next().unwrap_or(s);
    for f in ["%Y-%m-%d", "%Y/%m/%d", "%d.%m.%Y", "%m/%d/%Y", "%d/%m/%Y", "%Y%m%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(head, f) {
            return Some(d);
        }
    }
    // "2026-03" → first of the month
    if let Ok(d) = NaiveDate::parse_from_str(&format!("{head}-01"), "%Y-%m-%d") {
        return Some(d);
    }
    None
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Val>>,
}

impl Table {
    pub fn col(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name).or_else(|| {
            let l = name.to_lowercase();
            self.columns.iter().position(|c| c.to_lowercase() == l)
        })
    }

    /// Converts each column to its dominant type (numbers, dates, text).
    pub fn infer(mut self) -> Self {
        let n = self.columns.len();
        for c in 0..n {
            let mut nums = 0;
            let mut dates = 0;
            let mut seen = 0;
            // codes and day.month values ("04.02", "007") keep their leading zeros as text
            let mut zero_led = false;
            for r in self.rows.iter().take(500) {
                let v = r.get(c).unwrap_or(&Val::Null);
                if v.is_null() {
                    continue;
                }
                seen += 1;
                match v {
                    Val::Num(_) => nums += 1,
                    Val::Date(_) => dates += 1,
                    Val::Str(s) => {
                        if parse_date(s).is_some() && s.chars().any(|ch| ch == '-' || ch == '/' || ch == '.') && !s.contains(',') && s.parse::<f64>().is_err() {
                            dates += 1;
                        } else if parse_num(s).is_some() {
                            nums += 1;
                            let t = s.trim();
                            if t.len() > 1 && t.starts_with('0') && t.as_bytes()[1].is_ascii_digit() {
                                zero_led = true;
                            }
                        }
                    }
                    Val::Null => {}
                }
            }
            if seen == 0 {
                continue;
            }
            let to = if dates * 10 >= seen * 9 {
                Some("date")
            } else if nums * 10 >= seen * 9 && !zero_led {
                Some("number")
            } else {
                None
            };
            for r in self.rows.iter_mut() {
                if let Some(v) = r.get_mut(c) {
                    let nv = match (to, &*v) {
                        (_, Val::Null) => Val::Null,
                        (Some("date"), x) => x.date().map(Val::Date).unwrap_or(Val::Null),
                        (Some("number"), x) => x.num().map(Val::Num).unwrap_or(Val::Null),
                        (None, Val::Str(_)) => continue,
                        (None, x) => Val::Str(x.text()),
                        _ => continue,
                    };
                    *v = nv;
                }
            }
        }
        self
    }

    pub fn fields(&self) -> Vec<Field> {
        (0..self.columns.len())
            .map(|c| {
                let kind = self
                    .rows
                    .iter()
                    .map(|r| r.get(c).unwrap_or(&Val::Null))
                    .find(|v| !v.is_null())
                    .map(|v| match v {
                        Val::Num(_) => "number",
                        Val::Date(_) => "date",
                        _ => "text",
                    })
                    .unwrap_or("text");
                Field { name: self.columns[c].clone(), kind: kind.into() }
            })
            .collect()
    }

    /// The rows as {columns, rows} for Python (`sources["name"]`).
    pub fn to_json(&self) -> Json {
        json!({ "columns": self.columns, "rows": self.rows.iter().map(|r| r.iter().map(Val::to_json).collect::<Vec<_>>()).collect::<Vec<_>>() })
    }

    pub fn preview(&self, limit: usize) -> Json {
        json!({
            "columns": self.columns,
            "fields": self.fields(),
            "rows": self.rows.iter().take(limit).map(|r| r.iter().map(Val::to_json).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "total": self.rows.len(),
        })
    }

    /// Keeps rows whose date column falls in the range.
    pub fn filter_dates(&self, col: usize, r: &Range) -> Table {
        Table {
            columns: self.columns.clone(),
            rows: self
                .rows
                .iter()
                .filter(|row| row.get(col).and_then(|v| v.date()).map(|d| r.contains(d)).unwrap_or(false))
                .cloned()
                .collect(),
        }
    }
}

/// Everything a source needs besides its own definition.
pub struct LoadCtx<'a> {
    pub base_dir: Option<&'a Path>,
    pub range: Option<Range>,
    pub values: &'a BTreeMap<String, String>,
    pub py: &'a mut PyEngine,
    /// Other sources a Python script reads (`sources["name"]`): name → {columns, rows}.
    pub sources: Option<Json>,
}

/// Resolves a data file path: absolute, relative to the document, or `~/…`.
// ------------------------------------------------------------------ the web server's fence

/// Where data may come from on the web server: files only inside these folders, database servers
/// only these hosts (None: any). The desktop app sets none — the person's own computer.
struct Fence {
    dirs: Vec<PathBuf>,
    hosts: Option<Vec<String>>,
}

static FENCE: std::sync::OnceLock<Fence> = std::sync::OnceLock::new();

/// Fences data in (the web server, once at start).
pub fn fence(dirs: &[PathBuf], hosts: Option<Vec<String>>) {
    let dirs = dirs.iter().filter_map(|d| d.canonicalize().ok()).collect();
    let _ = FENCE.set(Fence { dirs, hosts: hosts.map(|h| h.into_iter().map(|x| x.to_lowercase()).collect()) });
}

fn fenced() -> Option<&'static Fence> {
    FENCE.get()
}

/// A file data may be read from (or written to).
pub fn check_file(p: &Path) -> Result<(), String> {
    let Some(f) = fenced() else { return Ok(()) };
    let real = p.canonicalize().map_err(|_| format!("{} doesn't exist on the server.", p.display()))?;
    if f.dirs.iter().any(|d| real.starts_with(d)) {
        Ok(())
    } else {
        Err(format!("{} is outside the folders Quire may read on this server. Upload the file, or ask the administrator to allow its folder.", p.display()))
    }
}

/// A database server data may be read from.
pub fn check_host(url: &str) -> Result<(), String> {
    let Some(hosts) = fenced().and_then(|f| f.hosts.as_ref()) else { return Ok(()) };
    let rest = url.split_once("://").map(|x| x.1).unwrap_or(url);
    let auth = rest.split(['/', '?']).next().unwrap_or("");
    let host = auth.rsplit_once('@').map(|x| x.1).unwrap_or(auth);
    let host = if host.starts_with('[') { host.split(']').next().unwrap_or("").trim_start_matches('[') } else { host.split(':').next().unwrap_or("") }.to_lowercase();
    if hosts.iter().any(|h| *h == host) {
        Ok(())
    } else {
        Err(format!("This server may not connect to {host}. The administrator can allow it (--allow-host)."))
    }
}

/// SQLite SQL may not attach other database files when data is fenced in.
fn check_sqlite_sql(sql: &str) -> Result<(), String> {
    if fenced().is_some() && sql.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "attach") {
        return Err("ATTACH isn't allowed on this server.".into());
    }
    Ok(())
}

pub fn resolve_path(p: &str, base: Option<&Path>) -> PathBuf {
    let path = if let Some(rest) = p.strip_prefix("~/") {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest)
    } else {
        PathBuf::from(p)
    };
    if path.is_absolute() {
        return path;
    }
    match base {
        Some(b) => b.join(path),
        None => path,
    }
}

pub fn load(src: &Source, ctx: &mut LoadCtx) -> Result<Table, String> {
    let t = match src {
        Source::Inline { columns, rows } => Table {
            columns: columns.clone(),
            rows: rows.iter().map(|r| r.iter().map(Val::from_json).collect()).collect(),
        },
        Source::Csv { path } => {
            let p = resolve_path(path, ctx.base_dir);
            check_file(&p)?;
            load_csv(&p)?
        }
        Source::Excel { path, sheet } => {
            let p = resolve_path(path, ctx.base_dir);
            check_file(&p)?;
            load_excel(&p, sheet)?
        }
        Source::Sqlite { path, query } => {
            let (sql, named) = bind_sql_named(query, ctx.range, ctx.values);
            let mut t = load_sqlite(&resolve_path(path, ctx.base_dir), &sql)?;
            name_param_columns(&mut t, &named);
            t
        }
        Source::Postgres { url, query } => load_postgres(url, &bind_sql(query, ctx.range, ctx.values))?,
        Source::Clickhouse { url, query } => {
            let (sql, named) = bind_sql_named(query, ctx.range, ctx.values);
            let mut t = load_clickhouse(url, &sql)?;
            name_param_columns(&mut t, &named);
            t
        }
        Source::Duckdb { path, query } => {
            let db = if path.trim().is_empty() { None } else { Some(resolve_path(path, ctx.base_dir)) };
            let (sql, named) = bind_sql_named(query, ctx.range, ctx.values);
            let mut t = load_duckdb(db.as_deref(), ctx.base_dir, &sql)?;
            name_param_columns(&mut t, &named);
            t
        }
        Source::Python { code } => load_python(code, ctx)?,
        // run by the query engine, which has the other datasets
        Source::Query { .. } => Table { columns: Vec::new(), rows: Vec::new() },
        // resolved to its connection's loader first (dbconn::resolve)
        Source::Db { .. } => return Err("This data source needs its connection.".into()),
    };
    // DuckDB columns come typed: text stays text ("+4 398 137 ₽" is a label, not a number)
    if matches!(src, Source::Duckdb { .. }) {
        return Ok(t);
    }
    Ok(t.infer())
}

fn load_csv(path: &Path) -> Result<Table, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let delim = [b',', b';', b'\t', b'|']
        .into_iter()
        .max_by_key(|d| first.matches(*d as char).count())
        .unwrap_or(b',');
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).flexible(true).from_reader(text.as_bytes());
    let columns: Vec<String> = rdr.headers().map_err(|e| e.to_string())?.iter().map(|s| s.trim().to_string()).collect();
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| e.to_string())?;
        let mut row: Vec<Val> =
            rec.iter().map(|s| if s.trim().is_empty() { Val::Null } else { Val::Str(s.to_string()) }).collect();
        row.resize(columns.len(), Val::Null);
        rows.push(row);
    }
    Ok(Table { columns, rows })
}

fn load_excel(path: &Path, sheet: &str) -> Result<Table, String> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut wb = open_workbook_auto(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let name = if sheet.is_empty() {
        wb.sheet_names().first().cloned().ok_or("The workbook has no sheets.")?
    } else {
        sheet.to_string()
    };
    let range = wb.worksheet_range(&name).map_err(|e| format!("Sheet '{name}': {e}"))?;
    let mut it = range.rows();
    let header = it.next().ok_or("The sheet is empty.")?;
    let columns: Vec<String> = header
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let s = c.to_string();
            if s.trim().is_empty() {
                format!("Column{}", i + 1)
            } else {
                s.trim().to_string()
            }
        })
        .collect();
    let epoch = NaiveDate::from_ymd_opt(EXCEL_EPOCH.0, EXCEL_EPOCH.1, EXCEL_EPOCH.2).unwrap();
    let rows = it
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Data::Empty => Val::Null,
                    Data::Int(i) => Val::Num(*i as f64),
                    Data::Float(f) => Val::Num(*f),
                    Data::Bool(b) => Val::Num(if *b { 1.0 } else { 0.0 }),
                    Data::String(s) => Val::Str(s.clone()),
                    Data::DateTime(d) => Val::Date(epoch + Duration::days(d.as_f64().floor() as i64)),
                    Data::DateTimeIso(s) => parse_date(s).map(Val::Date).unwrap_or(Val::Str(s.clone())),
                    Data::DurationIso(s) => Val::Str(s.clone()),
                    Data::Error(e) => Val::Str(format!("{e:?}")),
                })
                .collect()
        })
        .collect();
    Ok(Table { columns, rows })
}

/// Substitutes `:data_start`, `:data_end` (the range of data to load; the
/// older `:start`, `:end` still work) and `:param` placeholders with quoted literals.
pub fn bind_sql(query: &str, range: Option<Range>, values: &BTreeMap<String, String>) -> String {
    bind_sql_named(query, range, values).0
}

/// Gives a column named after a parameter's literal (`select :client` makes
/// DuckDB and SQLite call it `'Кирилл Шарихин'`) the parameter's name: `:client`.
pub fn name_param_columns(t: &mut Table, named: &[(String, String)]) {
    // in order: the first such column is the first parameter that gave that literal
    let mut used = vec![false; named.len()];
    for c in &mut t.columns {
        // a repeated name gets a suffix: '2026-01-01'_1
        let same = |lit: &str| c == lit || c.strip_prefix(lit).and_then(|r| r.strip_prefix('_')).is_some_and(|n| !n.is_empty() && n.chars().all(|d| d.is_ascii_digit()));
        if let Some(i) = named.iter().enumerate().position(|(i, (lit, _))| !used[i] && same(lit)) {
            used[i] = true;
            *c = format!(":{}", named[i].1);
        }
    }
}

/// `bind_sql`, and which literal each parameter became: (literal as written, name).
pub fn bind_sql_named(query: &str, range: Option<Range>, values: &BTreeMap<String, String>) -> (String, Vec<(String, String)>) {
    let mut named: Vec<(String, String)> = Vec::new();
    let mut vars: Vec<(String, String)> = Vec::new();
    if let Some(r) = range {
        let (s, e) = (r.start.format("%Y-%m-%d").to_string(), r.end.format("%Y-%m-%d").to_string());
        vars.push(("data_start".into(), s.clone()));
        vars.push(("data_end".into(), e.clone()));
        vars.push(("start".into(), s));
        vars.push(("end".into(), e));
    } else {
        for (k, v) in [("data_start", "0001-01-01"), ("data_end", "9999-12-31"), ("start", "0001-01-01"), ("end", "9999-12-31")] {
            vars.push((k.into(), v.into()));
        }
    }
    // explicit values win over the range's (SQL over sources sets :data_start itself)
    vars.retain(|(k, _)| !values.contains_key(k));
    for (k, v) in values {
        vars.push((k.clone(), v.clone()));
    }
    // longest names first so :end doesn't eat :end_date
    vars.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let mut out = String::with_capacity(query.len());
    let mut in_str = false;
    let mut prev = '\0';
    let mut it = query.char_indices();
    while let Some((i, ch)) = it.next() {
        // a comment runs to the end of its line: an apostrophe in it starts no string
        if !in_str && ch == '-' && query[i + 1..].starts_with('-') {
            let end = query[i..].find('\n').map(|k| i + k).unwrap_or(query.len());
            out.push_str(&query[i..end]);
            while it.clone().next().is_some_and(|(j, _)| j < end) {
                it.next();
            }
            prev = '-';
            continue;
        }
        if ch == '\'' {
            in_str = !in_str;
        }
        if !in_str && ch == ':' && prev != ':' && !query[i + 1..].starts_with(':') {
            let rest = &query[i + 1..];
            if let Some((k, v)) = vars.iter().find(|(k, _)| {
                rest.starts_with(k.as_str()) && !rest[k.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
            }) {
                // several values (a multi-choice parameter) become a list: client in (:client)
                let parts: Vec<&str> = if v.contains('\u{1f}') { v.split('\u{1f}').map(str::trim).filter(|x| !x.is_empty()).collect() } else { vec![v.as_str()] };
                let quoted: Vec<String> = parts.iter().map(|p| format!("'{}'", p.replace('\'', "''"))).collect();
                let lit = quoted.join(", ");
                named.push((lit.clone(), k.clone()));
                out.push_str(&lit);
                let stop = i + 1 + k.len();
                while it.clone().next().is_some_and(|(j, _)| j < stop) {
                    it.next();
                }
                prev = 'x';
                continue;
            }
        }
        out.push(ch);
        prev = ch;
    }
    (out, named)
}

fn load_sqlite(path: &Path, sql: &str) -> Result<Table, String> {
    check_file(path)?;
    check_sqlite_sql(sql)?;
    let conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let mut stmt = conn.prepare(sql).map_err(|e| format!("SQL error: {e}"))?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let n = columns.len();
    let mut rows = Vec::new();
    let mut q = stmt.query([]).map_err(|e| format!("SQL error: {e}"))?;
    while let Some(r) = q.next().map_err(|e| format!("SQL error: {e}"))? {
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            use rusqlite::types::ValueRef;
            row.push(match r.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => Val::Null,
                ValueRef::Integer(v) => Val::Num(v as f64),
                ValueRef::Real(v) => Val::Num(v),
                ValueRef::Text(t) => Val::Str(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(_) => Val::Str("<blob>".into()),
            });
        }
        rows.push(row);
    }
    Ok(Table { columns, rows })
}

thread_local! {
    /// The SQL workspace shows a timestamp's time; reports use its day.
    static KEEP_TIME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with timestamps keeping their time of day (the SQL workspace).
pub fn with_times<T>(f: impl FnOnce() -> T) -> T {
    KEEP_TIME.with(|k| k.set(true));
    let out = f();
    KEEP_TIME.with(|k| k.set(false));
    out
}

/// A date written the usual ways — 20261231, 31.12.2026, 31/12/2026, 2026/12/31,
/// 2026.12.31 (optionally with a time) — as an ISO date DuckDB reads.
fn loose_date(s: &str) -> Option<String> {
    let s = s.trim();
    let (d, time) = match s.split_once(|c: char| c == ' ' || c == 'T') {
        Some((d, t)) if t.chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.') && t.contains(':') => (d, Some(t)),
        _ => (s, None),
    };
    let num = |x: &str| x.parse::<u32>().ok();
    let (y, m, dd) = if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) {
        (num(&d[..4])?, num(&d[4..6])?, num(&d[6..])?)
    } else {
        let parts: Vec<&str> = d.split(|c| c == '.' || c == '/' || c == '-').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
            return None;
        }
        if parts[0].len() == 4 {
            (num(parts[0])?, num(parts[1])?, num(parts[2])?)
        } else if parts[2].len() == 4 {
            let (a, b) = (num(parts[0])?, num(parts[1])?);
            // day first, unless that can't be (12/31/2026)
            if a <= 12 && b > 12 { (num(parts[2])?, a, b) } else { (num(parts[2])?, b, a) }
        } else {
            return None;
        }
    };
    let date = NaiveDate::from_ymd_opt(y as i32, m, dd)?;
    Some(match time {
        Some(t) => format!("{} {t}", date.format("%Y-%m-%d")),
        None => date.format("%Y-%m-%d").to_string(),
    })
}

/// DuckDB SQL as people write it, made to run here: date strings in common
/// formats where a date is meant ('20261231'::date, date '31.12.2026',
/// cast('31/12/2026' as date)), and today's date and time without the
/// timezone extension (current_date, today(), current_timestamp, now()).
pub fn duckdb_compat(sql: &str) -> String {
    let now = chrono::Local::now();
    let today = format!("DATE '{}'", now.format("%Y-%m-%d"));
    let stamp = format!("TIMESTAMP '{}'", now.format("%Y-%m-%d %H:%M:%S"));
    let lower = sql.to_lowercase();
    let b = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 16);
    let mut i = 0;
    let word_at = |i: usize, w: &str| lower[i..].starts_with(w) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) && !lower[i + w.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_');
    while i < sql.len() {
        let c = b[i];
        if c == b'\'' {
            // a string literal: maybe a date in another format
            let mut j = i + 1;
            while j < sql.len() {
                if b[j] == b'\'' {
                    if j + 1 < sql.len() && b[j + 1] == b'\'' {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            let end = (j + 1).min(sql.len());
            let body = &sql[i + 1..j.min(sql.len())];
            let after = lower[end..].trim_start();
            let before = lower[..i].trim_end();
            let dated = after.starts_with("::date") || after.starts_with("::timestamp") || after.starts_with("as date") || after.starts_with("as timestamp")
                || before.ends_with("date") || before.ends_with("timestamp");
            match loose_date(body).filter(|_| dated) {
                Some(iso) => out.push_str(&format!("'{iso}'")),
                None => out.push_str(&sql[i..end]),
            }
            i = end;
            continue;
        }
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            // a comment runs to the end of the line
            let e = sql[i..].find('\n').map(|k| i + k).unwrap_or(sql.len());
            out.push_str(&sql[i..e]);
            i = e;
            continue;
        }
        let call = |w: &str| -> Option<usize> {
            if !word_at(i, w) {
                return None;
            }
            let rest = &lower[i + w.len()..];
            let t = rest.trim_start();
            t.starts_with("()").then(|| i + w.len() + (rest.len() - t.len()) + 2)
        };
        if let Some(e) = call("today") {
            out.push_str(&today);
            i = e;
        } else if let Some(e) = call("now").or_else(|| call("get_current_timestamp")) {
            out.push_str(&stamp);
            i = e;
        } else if word_at(i, "current_date") {
            out.push_str(&today);
            i += "current_date".len();
        } else if word_at(i, "current_timestamp") || word_at(i, "localtimestamp") {
            let w = if word_at(i, "current_timestamp") { "current_timestamp" } else { "localtimestamp" };
            out.push_str(&stamp);
            i += w.len();
        } else {
            let ch = sql[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn load_duckdb(db: Option<&Path>, base: Option<&Path>, sql: &str) -> Result<Table, String> {
    if let Some(p) = db {
        check_file(p)?;
    }
    let sql = &duckdb_compat(sql);
    // files are fine (that's the point of this source); extensions such as
    // httpfs are not, so a shared template can't send data anywhere
    let cfg = || duckdb::Config::default().enable_autoload_extension(false).map_err(|e| e.to_string());
    // a database file is opened once and shared (a report runs dozens of queries on it)
    if let Some(p) = db {
        return duck_query(&duck_shared(p, base)?, sql);
    }
    let conn = duckdb::Connection::open_in_memory_with_flags(cfg()?).map_err(|e| e.to_string())?;
    // relative file names in the query ('sales.parquet') resolve next to the document
    if let Some(dir) = base {
        let _ = conn.execute_batch(&format!("SET file_search_path = '{}'", dir.display().to_string().replace('\'', "''")));
    }
    // on the server, the files its SQL reads ('x.parquet', read_csv(…)) only from the allowed folders
    if let Some(f) = fenced() {
        let list = f.dirs.iter().map(|d| format!("'{}/'", d.display().to_string().replace('\'', "''"))).collect::<Vec<_>>().join(", ");
        conn.execute_batch(&format!("SET allowed_directories = [{list}]; SET enable_external_access = false; SET lock_configuration = true;"))
            .map_err(|e| format!("Could not fence the database in: {e}"))?;
    }
    duck_query(&conn, sql)
}

/// The open read-only DuckDB databases, by file: one instance each, opened again when the file
/// changes. Opening one reads its whole catalogue and starts its worker threads, which a report's
/// dozens of queries (and many people's reports) would otherwise do every time.
type Shared = std::collections::HashMap<(PathBuf, Option<PathBuf>), (Option<std::time::SystemTime>, duckdb::Connection)>;

fn shared_dbs() -> std::sync::MutexGuard<'static, Shared> {
    static DBS: std::sync::OnceLock<std::sync::Mutex<Shared>> = std::sync::OnceLock::new();
    DBS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// A connection to the shared instance of a database file (`base`: where relative file names in
/// its SQL resolve).
fn duck_shared(p: &Path, base: Option<&Path>) -> Result<duckdb::Connection, String> {
    let file = p.canonicalize().map_err(|e| format!("Could not open {}: {e}", p.display()))?;
    let dir = base.map(Path::to_path_buf).or_else(|| file.parent().map(Path::to_path_buf));
    let stamp = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
    let key = (file.clone(), dir.clone());
    let mut dbs = shared_dbs();
    if let Some((s, c)) = dbs.get(&key) {
        if *s == stamp {
            if let Ok(c) = c.try_clone() {
                return Ok(c);
            }
        }
    }
    let cfg = duckdb::Config::default()
        .enable_autoload_extension(false)
        .and_then(|c| c.access_mode(duckdb::AccessMode::ReadOnly))
        .map_err(|e| e.to_string())?;
    let conn = duckdb::Connection::open_with_flags(&file, cfg).map_err(|e| format!("Could not open {}: {e}", p.display()))?;
    if let Some(d) = &dir {
        let _ = conn.execute_batch(&format!("SET file_search_path = '{}'", d.display().to_string().replace('\'', "''")));
    }
    if let Some(f) = fenced() {
        let list = f.dirs.iter().map(|d| format!("'{}/'", d.display().to_string().replace('\'', "''"))).collect::<Vec<_>>().join(", ");
        conn.execute_batch(&format!("SET allowed_directories = [{list}]; SET enable_external_access = false; SET lock_configuration = true;"))
            .map_err(|e| format!("Could not fence the database in: {e}"))?;
    }
    let out = conn.try_clone().map_err(|e| e.to_string())?;
    dbs.insert(key, (stamp, conn));
    Ok(out)
}

/// Lets go of a database file's shared instances (before it is written to).
pub fn release_db(p: &Path) {
    let Ok(file) = p.canonicalize() else { return };
    shared_dbs().retain(|(f, _), _| *f != file);
}

/// A sandboxed in-memory DuckDB for SQL over loaded data: no files, no
/// network, no extensions — only the tables Quire puts in.
pub fn sandbox() -> Result<duckdb::Connection, String> {
    let cfg = duckdb::Config::default()
        .enable_autoload_extension(false)
        .and_then(|c| c.enable_external_access(false))
        .map_err(|e| e.to_string())?;
    let conn = duckdb::Connection::open_in_memory_with_flags(cfg).map_err(|e| e.to_string())?;
    let _ = conn.execute_batch("SET lock_configuration = true");
    Ok(conn)
}

pub fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// A dataset's table name in SQL: "Sales targets" → sales_targets.
pub fn sql_name(name: &str) -> String {
    let mut s: String = name.trim().to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() || s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("t_{s}")
    } else {
        s
    }
}

/// Loads a table into DuckDB under `name`, typed by its columns.
pub fn duck_register(conn: &duckdb::Connection, name: &str, t: &Table) -> Result<(), String> {
    use duckdb::types::Value as DV;
    let fields = t.fields();
    let cols: Vec<String> = fields
        .iter()
        .map(|f| format!("{} {}", quote_ident(&f.name), match f.kind.as_str() { "number" => "DOUBLE", "date" => "DATE", _ => "VARCHAR" }))
        .collect();
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} ({})", quote_ident(name), cols.join(", ")))
        .map_err(|e| format!("SQL error: {e}"))?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let mut app = conn.appender(name).map_err(|e| e.to_string())?;
    for r in &t.rows {
        let vals: Vec<DV> = fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let v = r.get(i).unwrap_or(&Val::Null);
                match (f.kind.as_str(), v) {
                    (_, Val::Null) => DV::Null,
                    ("number", v) => v.num().map(DV::Double).unwrap_or(DV::Null),
                    ("date", v) => v.date().map(|d| DV::Date32((d - epoch).num_days() as i32)).unwrap_or(DV::Null),
                    (_, v) => DV::Text(v.text()),
                }
            })
            .collect();
        app.append_row(duckdb::appender_params_from_iter(vals)).map_err(|e| e.to_string())?;
    }
    app.flush().map_err(|e| e.to_string())
}

/// Runs a query and reads the result into a table.
pub fn duck_query(conn: &duckdb::Connection, sql: &str) -> Result<Table, String> {
    use duckdb::types::{TimeUnit, ValueRef};
    let mut stmt = conn.prepare(sql).map_err(|e| format!("SQL error: {e}"))?;
    let mut rows = stmt.query([]).map_err(|e| format!("SQL error: {e}"))?;
    let columns = rows.as_ref().map(|s| s.column_names()).unwrap_or_default();
    let n = columns.len();
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let ts = |unit: TimeUnit, v: i64| -> Val {
        let secs = match unit {
            TimeUnit::Second => v,
            TimeUnit::Millisecond => v / 1_000,
            TimeUnit::Microsecond => v / 1_000_000,
            TimeUnit::Nanosecond => v / 1_000_000_000,
        };
        let keep = KEEP_TIME.with(|k| k.get());
        chrono::DateTime::from_timestamp(secs, 0)
            .map(|d| {
                let n = d.naive_utc();
                if keep && n.time() != chrono::NaiveTime::MIN {
                    Val::Str(n.format("%Y-%m-%d %H:%M:%S").to_string())
                } else {
                    Val::Date(n.date())
                }
            })
            .unwrap_or(Val::Null)
    };
    let mut out = Vec::new();
    while let Some(r) = rows.next().map_err(|e| format!("SQL error: {e}"))? {
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            row.push(match r.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => Val::Null,
                ValueRef::Boolean(b) => Val::Num(if b { 1.0 } else { 0.0 }),
                ValueRef::TinyInt(v) => Val::Num(v as f64),
                ValueRef::SmallInt(v) => Val::Num(v as f64),
                ValueRef::Int(v) => Val::Num(v as f64),
                ValueRef::BigInt(v) => Val::Num(v as f64),
                ValueRef::HugeInt(v) => Val::Num(v as f64),
                ValueRef::UHugeInt(v) => Val::Num(v as f64),
                ValueRef::UTinyInt(v) => Val::Num(v as f64),
                ValueRef::USmallInt(v) => Val::Num(v as f64),
                ValueRef::UInt(v) => Val::Num(v as f64),
                ValueRef::UBigInt(v) => Val::Num(v as f64),
                ValueRef::Float(v) => Val::Num(v as f64),
                ValueRef::Double(v) => Val::Num(v),
                ValueRef::Decimal(d) => d.to_string().parse().map(Val::Num).unwrap_or(Val::Null),
                ValueRef::Timestamp(u, v) => ts(u, v),
                ValueRef::Date32(d) => Val::Date(epoch + Duration::days(d as i64)),
                ValueRef::Text(t) => Val::Str(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Enum(..) | ValueRef::Time64(..) | ValueRef::Interval { .. } => Val::Str(r.get::<_, String>(i).unwrap_or_default()),
                _ => Val::Str("<complex>".into()),
            });
        }
        out.push(row);
    }
    Ok(Table { columns, rows: out })
}

fn load_postgres(url: &str, sql: &str) -> Result<Table, String> {
    check_host(url)?;
    use postgres::types::Type;
    let url = url.to_string();
    let sql = sql.to_string();
    // the sync client runs its own runtime; keep it off Tauri's async threads
    std::thread::spawn(move || -> Result<Table, String> {
        let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| format!("Could not connect: {e}"))?;
        let rows = client.query(sql.as_str(), &[]).map_err(|e| format!("SQL error: {e}"))?;
        let columns: Vec<String> = match rows.first() {
            Some(r) => r.columns().iter().map(|c| c.name().to_string()).collect(),
            None => client
                .prepare(sql.as_str())
                .map(|s| s.columns().iter().map(|c| c.name().to_string()).collect())
                .unwrap_or_default(),
        };
        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            let mut row = Vec::with_capacity(columns.len());
            for (i, c) in r.columns().iter().enumerate() {
                let t = c.type_();
                let v = if *t == Type::INT2 {
                    r.try_get::<_, Option<i16>>(i).ok().flatten().map(|v| Val::Num(v as f64))
                } else if *t == Type::INT4 {
                    r.try_get::<_, Option<i32>>(i).ok().flatten().map(|v| Val::Num(v as f64))
                } else if *t == Type::INT8 {
                    r.try_get::<_, Option<i64>>(i).ok().flatten().map(|v| Val::Num(v as f64))
                } else if *t == Type::FLOAT4 {
                    r.try_get::<_, Option<f32>>(i).ok().flatten().map(|v| Val::Num(v as f64))
                } else if *t == Type::FLOAT8 {
                    r.try_get::<_, Option<f64>>(i).ok().flatten().map(Val::Num)
                } else if *t == Type::BOOL {
                    r.try_get::<_, Option<bool>>(i).ok().flatten().map(|v| Val::Num(if v { 1.0 } else { 0.0 }))
                } else if *t == Type::DATE {
                    r.try_get::<_, Option<NaiveDate>>(i).ok().flatten().map(Val::Date)
                } else if *t == Type::TIMESTAMP {
                    r.try_get::<_, Option<chrono::NaiveDateTime>>(i).ok().flatten().map(|v| Val::Date(v.date()))
                } else if *t == Type::TIMESTAMPTZ {
                    r.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(i)
                        .ok()
                        .flatten()
                        .map(|v| Val::Date(v.date_naive()))
                } else if *t == Type::NUMERIC {
                    // NUMERIC has no lossless native mapping; cast in SQL (::float8) for speed
                    r.try_get::<_, Option<String>>(i).ok().flatten().map(Val::Str)
                } else {
                    r.try_get::<_, Option<String>>(i).ok().flatten().map(Val::Str)
                };
                row.push(v.unwrap_or(Val::Null));
            }
            out.push(row);
        }
        Ok(Table { columns, rows: out })
    })
    .join()
    .map_err(|_| "The database client crashed.".to_string())?
}

/// Where a ClickHouse connection points: its HTTP endpoint (with the database and output settings) and the
/// user and password, which travel as headers. `clickhouse://` (and `http://`) is plain HTTP on port 8123 by
/// default; `clickhouses://` (and `https://`) is TLS on 8443.
pub(crate) struct ChTarget {
    pub endpoint: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

pub(crate) fn clickhouse_target(url: &str) -> Result<ChTarget, String> {
    let url = url.trim();
    let (tls, rest) = if let Some(r) = url.strip_prefix("clickhouses://").or_else(|| url.strip_prefix("https://")) {
        (true, r)
    } else if let Some(r) = url.strip_prefix("clickhouse://").or_else(|| url.strip_prefix("http://")) {
        (false, r)
    } else {
        return Err("Write the server as clickhouse://user:password@host:8123/database (clickhouses:// for TLS).".into());
    };
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (auth, hostpath) = match rest.rsplit_once('@') {
        Some((a, h)) => (Some(a), h),
        None => (None, rest),
    };
    let (host, db) = hostpath.split_once('/').unwrap_or((hostpath, ""));
    if host.is_empty() {
        return Err("The ClickHouse address has no server name.".into());
    }
    let host = if host.contains(':') && !host.ends_with(']') { host.to_string() } else { format!("{host}:{}", if tls { 8443 } else { 8123 }) };
    let dec = |s: &str| percent_decode(s);
    let (user, password) = match auth {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (Some(dec(u)), Some(dec(p))),
            None => (Some(dec(a)), None),
        },
        None => (None, None),
    };
    let mut params = vec![
        "default_format=JSONCompact".to_string(),
        // whole numbers and decimals as JSON numbers, not quoted strings
        "output_format_json_quote_64bit_integers=0".into(),
        "output_format_json_quote_decimals=0".into(),
    ];
    let db = db.trim_end_matches('/');
    if !db.is_empty() {
        params.push(format!("database={db}"));
    }
    params.extend(query.split('&').filter(|q| !q.is_empty()).map(String::from));
    Ok(ChTarget { endpoint: format!("{}://{host}/?{}", if tls { "https" } else { "http" }, params.join("&")), user, password })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A string as curl's config file writes it: in double quotes, with \\ and \" escaped.
fn curl_quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// ClickHouse over its HTTP interface, through the system's curl (as the AI providers are reached): the query
/// is posted, the answer comes back as JSONCompact. The password goes in curl's config on stdin, never on its
/// command line.
fn load_clickhouse(url: &str, sql: &str) -> Result<Table, String> {
    check_host(url)?;
    use std::io::Write;
    use std::process::{Command, Stdio};
    let t = clickhouse_target(url)?;
    let mut cfg = format!("url = {}\nrequest = \"POST\"\nsilent\nshow-error\nmax-time = 300\nconnect-timeout = 15\nwrite-out = \"\\n%{{http_code}}\"\n", curl_quote(&t.endpoint));
    if let Some(u) = &t.user {
        cfg += &format!("header = {}\n", curl_quote(&format!("X-ClickHouse-User: {u}")));
    }
    if let Some(p) = &t.password {
        cfg += &format!("header = {}\n", curl_quote(&format!("X-ClickHouse-Key: {p}")));
    }
    cfg += "header = \"Content-Type: text/plain; charset=utf-8\"\n";
    cfg += &format!("data-binary = {}\n", curl_quote(sql.trim().trim_end_matches(';')));
    let mut child = Command::new("curl").args(["-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("curl is not available: {e}"))?;
    child.stdin.take().ok_or("curl stdin")?.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    let code: u16 = code.trim().parse().unwrap_or(0);
    if code == 0 {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("Could not connect: {}", if err.is_empty() { "no answer from the server" } else { err.as_str() }));
    }
    if code != 200 {
        // "Code: 60. DB::Exception: Unknown table … (UNKNOWN_TABLE) (version …)"
        let msg = body.trim().lines().next().unwrap_or("").trim();
        return Err(match code {
            401 | 403 if msg.is_empty() => "Could not connect: the server refused the user or password.".into(),
            _ if msg.is_empty() => format!("The server answered {code}."),
            _ => format!("SQL error: {msg}"),
        });
    }
    clickhouse_table(body)
}

/// A JSONCompact answer as a table: numbers, dates (a timestamp keeps its time in the SQL workspace) and text.
pub(crate) fn clickhouse_table(body: &str) -> Result<Table, String> {
    let j: Json = serde_json::from_str(body).map_err(|e| format!("Unexpected answer from ClickHouse: {e}"))?;
    let meta = j["meta"].as_array().ok_or("Unexpected answer from ClickHouse: no columns.")?;
    let columns: Vec<String> = meta.iter().map(|m| m["name"].as_str().unwrap_or("").to_string()).collect();
    // Nullable(…) and LowCardinality(…) wrap the type that matters
    let base = |t: &str| -> String {
        let mut t = t.trim();
        loop {
            let inner = ["Nullable(", "LowCardinality("].iter().find_map(|w| t.strip_prefix(w).and_then(|r| r.strip_suffix(')')));
            match inner {
                Some(i) => t = i.trim(),
                None => return t.to_string(),
            }
        }
    };
    let kinds: Vec<String> = meta.iter().map(|m| base(m["type"].as_str().unwrap_or(""))).collect();
    let keep = KEEP_TIME.with(|k| k.get());
    let cell = |v: &Json, k: &str| -> Val {
        if v.is_null() {
            return Val::Null;
        }
        let numeric = k.starts_with("Int") || k.starts_with("UInt") || k.starts_with("Float") || k.starts_with("Decimal") || k == "Bool";
        if numeric {
            return match v {
                Json::Number(n) => n.as_f64().map(Val::Num).unwrap_or(Val::Null),
                Json::Bool(b) => Val::Num(if *b { 1.0 } else { 0.0 }),
                Json::String(s) => s.parse::<f64>().map(Val::Num).unwrap_or_else(|_| Val::Str(s.clone())),
                _ => Val::Str(v.to_string()),
            };
        }
        let s = match v {
            Json::String(s) => s.clone(),
            other => other.to_string(),
        };
        if k.starts_with("Date") {
            let (d, time) = s.split_once(' ').unwrap_or((s.as_str(), ""));
            return match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                Ok(_) if keep && !time.is_empty() && time.trim_start_matches(['0', ':', '.']) != "" => Val::Str(s.clone()),
                Ok(day) => Val::Date(day),
                Err(_) => Val::Str(s),
            };
        }
        Val::Str(s)
    };
    let rows = j["data"]
        .as_array()
        .map(|rows| rows.iter().map(|r| r.as_array().map(|c| c.iter().zip(&kinds).map(|(v, k)| cell(v, k)).collect()).unwrap_or_default()).collect())
        .unwrap_or_default();
    Ok(Table { columns, rows })
}

fn load_python(code: &str, ctx: &mut LoadCtx) -> Result<Table, String> {
    let mut params = serde_json::Map::new();
    if let Some(r) = ctx.range {
        params.insert("data_start".into(), json!(r.start.format("%Y-%m-%d").to_string()));
        params.insert("data_end".into(), json!(r.end.format("%Y-%m-%d").to_string()));
        params.insert("start".into(), json!(r.start.format("%Y-%m-%d").to_string()));
        params.insert("end".into(), json!(r.end.format("%Y-%m-%d").to_string()));
    }
    for (k, v) in ctx.values {
        params.insert(k.clone(), json!(v));
    }
    let base = ctx.base_dir.map(|p| p.display().to_string());
    let res = ctx.py.dataset(code, &Json::Object(params), base.as_deref(), ctx.sources.as_ref());
    if !res.ok {
        return Err(res.error.unwrap_or_else(|| "Python failed.".into()));
    }
    let columns: Vec<String> = res
        .value
        .get("columns")
        .and_then(|c| c.as_array())
        .map(|a| a.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect())
        .unwrap_or_default();
    let rows = res
        .value
        .get("rows")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .map(|r| r.as_array().map(|r| r.iter().map(Val::from_json).collect()).unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();
    if columns.is_empty() {
        return Err("The script must leave a DataFrame (or list of dicts) in `df` or as its last expression.".into());
    }
    Ok(Table { columns, rows })
}

#[cfg(test)]
mod compat_tests {
    use super::*;

    #[test]
    fn dates_people_write_are_read() {
        assert_eq!(loose_date("20261231").as_deref(), Some("2026-12-31"));
        assert_eq!(loose_date("31.12.2026").as_deref(), Some("2026-12-31"));
        assert_eq!(loose_date("31/12/2026").as_deref(), Some("2026-12-31"));
        assert_eq!(loose_date("12/31/2026").as_deref(), Some("2026-12-31"));
        assert_eq!(loose_date("2026/12/31").as_deref(), Some("2026-12-31"));
        assert_eq!(loose_date("31.12.2026 10:30").as_deref(), Some("2026-12-31 10:30"));
        assert!(loose_date("hello").is_none() && loose_date("20261341").is_none());
        let q = duckdb_compat("select '20261231'::date, date '31.12.2026', cast('2026/12/31' as date), '20261231' as code, 'x::date' -- current_date\n, current_date, now(), today()");
        assert!(q.contains("'2026-12-31'::date") && q.contains("date '2026-12-31'") && q.contains("cast('2026-12-31' as date)"), "{q}");
        assert!(q.contains("'20261231' as code"), "a plain string stays: {q}");
        assert!(q.contains("-- current_date"), "comments stay: {q}");
        assert!(!q.contains("now()") && !q.contains("today()") && q.contains("TIMESTAMP '") && q.matches("DATE '").count() == 2, "{q}");
        // and DuckDB runs it
        let t = load_duckdb(None, None, "select '20261231'::date as a, date '31.12.2026' as b, current_date as c, now() as d, today() + 1 as e").unwrap();
        assert_eq!(t.rows.len(), 1);
        assert!(matches!(t.rows[0][0], Val::Date(_)) && matches!(t.rows[0][2], Val::Date(_)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_placeholders_outside_strings() {
        let mut vals = BTreeMap::new();
        vals.insert("region".to_string(), "North".to_string());
        let r = Range { start: crate::period::ymd(2026, 1, 1), end: crate::period::ymd(2026, 1, 31) };
        let renamed = bind_sql("select :data_start, :data_end", Some(Range { start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), end: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap() }), &BTreeMap::new());
        assert_eq!(renamed, "select '2026-01-01', '2026-01-31'");
        let sql = "select * from t where d between :start and :end and region = :region and note <> ':start' and x::text = 'a'";
        assert_eq!(
            bind_sql(sql, Some(r), &vals),
            "select * from t where d between '2026-01-01' and '2026-01-31' and region = 'North' and note <> ':start' and x::text = 'a'"
        );
    }

    #[test]
    fn sql_over_loaded_tables_is_sandboxed() {
        let conn = sandbox().unwrap();
        let t = Table {
            columns: vec!["d".into(), "region".into(), "amount".into()],
            rows: vec![
                vec![Val::Date(crate::period::ymd(2024, 1, 31)), Val::Str("North".into()), Val::Num(10.0)],
                vec![Val::Date(crate::period::ymd(2024, 2, 29)), Val::Str("South".into()), Val::Num(5.0)],
            ],
        };
        duck_register(&conn, "data", &t).unwrap();
        let r = duck_query(&conn, "select region, amount * 2 as double_amount, date_trunc('quarter', d) as q from data order by region").unwrap();
        assert_eq!(r.columns, vec!["region", "double_amount", "q"]);
        assert_eq!(r.rows[0], vec![Val::Str("North".into()), Val::Num(20.0), Val::Date(crate::period::ymd(2024, 1, 1))]);
        // no reaching outside the sandbox
        assert!(duck_query(&conn, "select * from read_csv('/etc/hosts')").is_err());
        assert!(conn.execute_batch("SET enable_external_access = true").is_err());
        assert_eq!(sql_name("Sales targets 2024"), "sales_targets_2024");
    }

    #[test]
    fn reads_numbers_as_written() {
        assert_eq!(parse_num("1,234.5"), Some(1234.5));
        assert_eq!(parse_num("$1,200"), Some(1200.0));
        assert_eq!(parse_num("12,5"), Some(12.5));
        assert_eq!(parse_num("1.234,56"), Some(1234.56));
        assert_eq!(parse_num("(300)"), Some(-300.0));
        assert_eq!(parse_num("45%"), Some(0.45));
        assert_eq!(parse_num("Q1"), None);
        assert_eq!(parse_num("BUY"), None);
        assert_eq!(Val::Num(1_758_000_000.0).date(), Some(crate::period::ymd(2025, 9, 16)));
    }

    #[test]
    fn infers_types() {
        let t = Table {
            columns: vec!["d".into(), "n".into(), "s".into()],
            rows: vec![
                vec![Val::Str("2026-01-05".into()), Val::Str("1,200.5".into()), Val::Str("North".into())],
                vec![Val::Str("2026-02-05".into()), Val::Str("3".into()), Val::Str("South".into())],
            ],
        }
        .infer();
        assert_eq!(t.rows[0][0], Val::Date(crate::period::ymd(2026, 1, 5)));
        assert_eq!(t.rows[0][1], Val::Num(1200.5));
        assert_eq!(t.rows[1][2], Val::Str("South".into()));
    }
}

#[cfg(test)]
mod clickhouse_tests {
    use super::*;

    #[test]
    fn clickhouse_addresses_become_endpoints_and_headers() {
        let t = clickhouse_target("clickhouse://analyst:p%40ss@dwh.bank.local/finance").unwrap();
        assert!(t.endpoint.starts_with("http://dwh.bank.local:8123/?default_format=JSONCompact"), "{}", t.endpoint);
        assert!(t.endpoint.ends_with("&database=finance"), "{}", t.endpoint);
        assert_eq!((t.user.as_deref(), t.password.as_deref()), (Some("analyst"), Some("p@ss")));
        let t = clickhouse_target("clickhouses://ro@ch.example.com:9443?session_timezone=Europe/Moscow").unwrap();
        assert!(t.endpoint.starts_with("https://ch.example.com:9443/?") && t.endpoint.ends_with("&session_timezone=Europe/Moscow"), "{}", t.endpoint);
        assert_eq!((t.user.as_deref(), t.password), (Some("ro"), None));
        assert!(clickhouse_target("postgresql://x@y/z").is_err());
    }

    #[test]
    fn clickhouse_answers_become_tables() {
        let body = r#"{"meta":[{"name":"client","type":"LowCardinality(String)"},{"name":"value","type":"Nullable(Decimal(18, 2))"},
            {"name":"n","type":"UInt64"},{"name":"d","type":"Date"},{"name":"at","type":"DateTime"},{"name":"ok","type":"Bool"}],
            "data":[["Alpha",1250.5,3,"2026-06-30","2026-06-30 14:05:00",true],["Beta",null,"18446744073709551615","2026-07-01","2026-07-01 00:00:00",false]],"rows":2}"#;
        let t = clickhouse_table(body).unwrap();
        assert_eq!(t.columns, ["client", "value", "n", "d", "at", "ok"]);
        assert_eq!(t.rows[0][0].text(), "Alpha");
        assert_eq!(t.rows[0][1].num(), Some(1250.5));
        assert!(matches!(t.rows[1][1], Val::Null));
        assert!(t.rows[1][2].num().is_some(), "a quoted UInt64 is still a number");
        assert!(matches!(t.rows[0][3], Val::Date(_)) && matches!(t.rows[0][4], Val::Date(_)), "reports use the day");
        assert_eq!(t.rows[0][5].num(), Some(1.0));
        let t = with_times(|| clickhouse_table(body).unwrap());
        assert_eq!(t.rows[0][4].text(), "2026-06-30 14:05:00", "the SQL workspace keeps the time");
        assert!(matches!(t.rows[1][4], Val::Date(_)), "midnight stays a date");
    }

    /// A tiny stand-in for ClickHouse's HTTP interface: answers one request and hands back what it got.
    fn fake_server(status: &'static str, answer: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let len = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if body.len() >= len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len());
            s.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&got).to_string()
        });
        (format!("clickhouse://reporter:s3cr%22t@127.0.0.1:{port}/bank"), h)
    }

    #[test]
    fn a_clickhouse_source_loads_over_http() {
        let (url, h) = fake_server("200 OK", r#"{"meta":[{"name":"client","type":"String"},{"name":"value","type":"Float64"}],"data":[["Alpha",10.5],["Beta",7]]}"#);
        let values = BTreeMap::from([("client".to_string(), "Alpha".to_string())]);
        let mut py = PyEngine::new();
        let mut ctx = LoadCtx { base_dir: None, range: None, values: &values, py: &mut py, sources: None };
        let t = load(&Source::Clickhouse { url, query: "select client, value from positions where client = :client;".into() }, &mut ctx).unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1][1].num(), Some(7.0));
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /?default_format=JSONCompact") && req.contains("database=bank"), "{req}");
        assert!(req.contains("X-ClickHouse-User: reporter") && req.contains("X-ClickHouse-Key: s3cr\"t"), "{req}");
        assert!(req.ends_with("select client, value from positions where client = 'Alpha'"), "the parameter is filled in, the ; dropped: {req}");
    }

    #[test]
    fn clickhouse_errors_say_what_the_server_said() {
        let (url, h) = fake_server("404 Not Found", "Code: 60. DB::Exception: Unknown table expression identifier 'positionz'. (UNKNOWN_TABLE)\n");
        let values = BTreeMap::new();
        let mut py = PyEngine::new();
        let mut ctx = LoadCtx { base_dir: None, range: None, values: &values, py: &mut py, sources: None };
        let e = load(&Source::Clickhouse { url, query: "select * from positionz".into() }, &mut ctx).unwrap_err();
        h.join().unwrap();
        assert!(e.starts_with("SQL error: Code: 60.") && e.contains("UNKNOWN_TABLE"), "{e}");
    }
}
