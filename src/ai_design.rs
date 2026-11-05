//! The designer's AI assistant: builds a page from a description, or changes the page in
//! front (add, remove, rearrange, switch chart types or design views). The model sees the
//! template's structure only — the page grid, sources with their field names and types, the
//! widget types and the theme's design views, and the widgets already on the page — never data.
//! It answers with a page plan; the app checks it against the template, applies it as one
//! undo step and hands back what didn't work (a missing field, a widget that doesn't fit).

use crate::model::{Binding, Doc, Measure, Page, Widget};
use serde_json::{json, Value as Json};

/// The widget types the assistant may use, with what each is for.
pub const KINDS: &[(&str, &str)] = &[
    ("kpi", "a headline number (first measure) with optional extra measures shown as a breakdown"),
    ("bar", "compare categories or periods; opts.orientation \"vertical\" | \"horizontal\""),
    ("line", "a trend over time (dims: a date field with a grain, e.g. \"date:month\")"),
    ("area", "volume over time"),
    ("pie", "shares of a total (donut)"),
    ("table", "exact figures by category, or rows as they are (binding.mode \"rows\" with binding.columns)"),
    ("treemap", "shares as areas"),
    ("waterfall", "steps that add up to a total"),
    ("tiles", "a card per category"),
    ("heading", "a page or section title: \"text\""),
    ("text", "a paragraph: \"text\""),
    ("divider", "a thin rule between sections (h 1, or w 1 for a vertical one)"),
];
pub const DATA_KINDS: &[&str] = &["kpi", "bar", "line", "area", "pie", "table", "treemap", "waterfall", "tiles"];
const AGGS: &[&str] = &["sum", "closing", "opening", "avg_daily", "avg", "count", "distinct", "min", "max"];
const WINDOWS: &[&str] = &["period", "ytd", "trailing3", "trailing6", "trailing12", "trailing24", "all"];
const FORMATS: &[&str] = &["number", "currency", "percent", "compact"];
const CALCS: &[&str] = &["", "prev", "delta", "pct", "share", "rank", "running", "trend"];
const GRAINS: &[&str] = &["month", "quarter", "year", "week", "day"];
const SORTS: &[&str] = &["", "value_desc", "value_asc", "dim_asc", "dim_desc", "data"];

fn lang_name(locale: &str) -> &'static str {
    if locale.starts_with("ru") { "Russian" } else { "English" }
}

/// A widget as the model sees it: what it shows and where — no data.
fn widget_brief(w: &Widget) -> Json {
    let mut o = json!({"id": w.id, "kind": w.kind, "x": w.x, "y": w.y, "w": w.w, "h": w.h});
    if !w.title.is_empty() { o["title"] = json!(w.title); }
    if !w.subtitle.is_empty() { o["subtitle"] = json!(w.subtitle); }
    if !w.variant.is_empty() { o["variant"] = json!(w.variant); }
    if let Some(t) = w.opts.get("text").and_then(|t| t.as_str()) { o["text"] = json!(t.chars().take(300).collect::<String>()); }
    if let Some(b) = &w.binding {
        let ms: Vec<Json> = b.measures.iter().map(|m| {
            let mut x = json!({"field": m.field, "agg": m.agg});
            if !m.label.is_empty() { x["label"] = json!(m.label); }
            if !m.format.is_empty() && m.format != "number" { x["format"] = json!(m.format); }
            if !m.calc.is_empty() { x["calc"] = json!(m.calc); }
            x
        }).collect();
        let mut bj = json!({"dataset": b.dataset, "dims": b.dims, "measures": ms, "window": b.window});
        if !b.sort.is_empty() { bj["sort"] = json!(b.sort); }
        if b.limit > 0 { bj["limit"] = json!(b.limit); bj["others"] = json!(b.others); }
        if b.mode == "rows" { bj["mode"] = json!("rows"); bj["columns"] = json!(b.columns); }
        o["binding"] = bj;
    }
    if w.kind == "bar" {
        if let Some(v) = w.opts.get("orientation") { o["orientation"] = v.clone(); }
    }
    o
}

/// What the assistant may know of the template: its grid, sources (names and types), widget
/// types, design views, report parameters — and, when changing a page, what is on it.
pub fn context(doc: &Doc, page: Option<&Page>) -> String {
    let g = crate::vm::grid(doc);
    let mut s = format!(
        "PAGE: {} {}, a grid of {} columns × {} rows (x 0–{}, y 0–{}). Widgets sit on whole cells, must stay inside the grid and must not overlap.\n",
        doc.page.size, if doc.page.landscape { "landscape" } else { "portrait" }, g.cols, g.rows, g.cols - 1, g.rows - 1
    );
    s.push_str(&format!("REPORT LANGUAGE: {} — write titles and texts in it.\n\n", lang_name(&doc.theme.locale)));
    s.push_str("SOURCES (id \"name\" [dated by field] [snapshot = balances: use agg closing/opening/avg_daily, not sum]: field type, …):\n");
    for ds in doc.datasets.iter().take(60) {
        let mut fields: Vec<String> = ds.fields.iter().take(80).map(|f| format!("{} {}", f.name, f.kind)).collect();
        fields.extend(ds.calc_fields.iter().map(|c| format!("{} calc", c.name)));
        s.push_str(&format!(
            "- {} \"{}\"{}{}: {}\n",
            ds.id, ds.name,
            ds.date_field.as_deref().filter(|d| !d.is_empty()).map(|d| format!(" [dated by {d}]")).unwrap_or_default(),
            if ds.snapshot { " [snapshot]" } else { "" },
            fields.join(", ")
        ));
    }
    if doc.datasets.is_empty() {
        s.push_str("(none yet — use only heading, text and divider)\n");
    }
    s.push_str("\nWIDGET TYPES (kind — use) and their DESIGN VIEWS (variant id: name — when to use it):\n");
    for (k, what) in KINDS {
        s.push_str(&format!("- {k} — {what}\n"));
        for v in doc.theme.styles.get(*k).into_iter().flatten() {
            let d = v.description.trim();
            s.push_str(&format!("    {}: {}{}\n", v.id, v.name, if d.is_empty() { String::new() } else { format!(" — {}", d.chars().take(160).collect::<String>()) }));
        }
    }
    let params: Vec<String> = doc.param_defs.iter().map(|p| format!("{{{{{}}}}} ({})", p.name, if p.label.is_empty() { &p.name } else { &p.label })).collect();
    s.push_str(&format!(
        "\nTEXT PLACEHOLDERS for titles and texts: {{{{period}}}}, {{{{start}}}}, {{{{end}}}}, {{{{compare}}}}, {{{{title}}}}, {{{{organization}}}}{}{}\n",
        if params.is_empty() { "" } else { ", " },
        params.join(", ")
    ));
    if let Some(p) = page {
        let ws: Vec<Json> = p.widgets.iter().map(widget_brief).collect();
        s.push_str(&format!("\nTHE CURRENT PAGE \"{}\" holds:\n{}\n", p.title, serde_json::to_string(&ws).unwrap_or_default()));
    }
    s
}

/// The system prompt: how to answer with a page plan.
pub fn system(ctx: &str, ui_lang: &str) -> String {
    let explain = if ui_lang == "ru" { "Russian" } else { "English" };
    format!(
        "You are the design assistant of Quire, an app a private bank's designers use to build client report templates.\n\
         You lay out report pages from the widgets below. You know the template's structure only — never its data.\n\n{ctx}\n\
         HOW A WIDGET IS WRITTEN\n\
         {{\"kind\": \"bar\", \"x\": 0, \"y\": 2, \"w\": 12, \"h\": 8, \"title\": \"…\", \"subtitle\": \"\", \"variant\": \"<a design view id of that kind, or omit for the default>\",\n\
          \"binding\": {{\"dataset\": \"<source id>\", \"dims\": [\"field\" or \"datefield:month\"], \"measures\": [{{\"field\": \"<number field, or * with agg count>\", \"agg\": \"sum\", \"label\": \"\", \"format\": \"number\", \"calc\": \"\"}}],\n\
          \"window\": \"period\", \"sort\": \"value_desc\", \"limit\": 0, \"others\": false}}, \"orientation\": \"horizontal\"}}\n\
         heading/text/divider have no binding; heading and text carry \"text\".\n\
         - agg: {aggs}. window: {windows}. format: {formats}. calc (on a measure): prev, delta, pct (change vs the comparison period), share, rank, running, trend (sparkline). sort: value_desc, value_asc, dim_asc, dim_desc. Grains: {grains}.\n\
         - Use only sources and fields listed. A KPI has no dims. Line/area charts group by a date field with a grain. Pie/treemap: one dim, one measure, at most ~6 slices (limit 5 + others true).\n\
         - Pick the design view whose name and description fit the request; the theme gives the look, so don't invent colours or fonts.\n\
         - Lay out like a designer: a heading row on top, headline KPIs in a row of equal cards, then charts and tables on a clear grid; fill the page without overlaps; KPI cards at least 3 rows high, charts at least 6, headings 1–2.\n\n\
         ANSWER with ONE JSON object and nothing else:\n\
         {{\"explanation\": \"1–3 short sentences in {explain} on what you built or changed\", \"title\": \"the page title (report language)\", \"widgets\": [ … ]}}\n\
         - To build a new page, list every widget of it.\n\
         - To change the current page, list EVERY widget the page should have afterwards: a kept widget carries its \"id\" and only the fields that change (omitted fields stay as they are); a new widget has no id; a widget left out is removed.",
        aggs = AGGS.join(", "),
        windows = WINDOWS.join(", "),
        formats = FORMATS.join(", "),
        grains = GRAINS.join(", "),
    )
}

/// The page plan turned into widgets, checked against the template. Problems go back to the model.
#[derive(Debug)]
pub struct Plan {
    pub title: String,
    pub widgets: Vec<Widget>,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
}

fn field_ok(doc: &Doc, ds: &str, f: &str) -> bool {
    doc.dataset(ds).is_some_and(|d| d.fields.iter().any(|x| x.name == f) || d.calc_fields.iter().any(|c| c.name == f) || d.date_field.as_deref() == Some(f))
}

/// Reads the model's binding into `b` (only the keys it gives), checking every name.
fn read_binding(doc: &Doc, j: &Json, b: &mut Binding, who: &str, probs: &mut Vec<String>) {
    if let Some(d) = j["dataset"].as_str() {
        match doc.datasets.iter().find(|x| x.id == d || x.name == d) {
            Some(ds) => b.dataset = ds.id.clone(),
            None => probs.push(format!("{who}: there is no source \"{d}\".")),
        }
    }
    if let Some(a) = j["dims"].as_array() {
        b.dims.clear();
        for d in a.iter().filter_map(|x| x.as_str()) {
            let (f, grain) = d.split_once(':').unwrap_or((d, ""));
            if !grain.is_empty() && !GRAINS.contains(&grain) {
                probs.push(format!("{who}: \"{grain}\" is not a grain ({}).", GRAINS.join(", ")));
            }
            if !b.dataset.is_empty() && !field_ok(doc, &b.dataset, f) {
                probs.push(format!("{who}: source {} has no field \"{f}\".", b.dataset));
            }
            b.dims.push(d.to_string());
        }
    }
    if let Some(a) = j["measures"].as_array() {
        let old = std::mem::take(&mut b.measures);
        for (i, m) in a.iter().enumerate() {
            let mut x = old.get(i).cloned().unwrap_or_else(|| Measure { format: "number".into(), agg: "sum".into(), ..Default::default() });
            if let Some(f) = m["field"].as_str() { x.field = f.to_string(); }
            if let Some(v) = m["agg"].as_str() { x.agg = v.to_string(); }
            if let Some(v) = m["label"].as_str() { x.label = v.to_string(); }
            if let Some(v) = m["format"].as_str() { x.format = v.to_string(); }
            if let Some(v) = m["calc"].as_str() { x.calc = v.to_string(); }
            if x.field == "*" && x.agg != "count" { x.agg = "count".into(); }
            if x.field != "*" && !b.dataset.is_empty() && !field_ok(doc, &b.dataset, &x.field) {
                probs.push(format!("{who}: source {} has no field \"{}\".", b.dataset, x.field));
            }
            if !AGGS.contains(&x.agg.as_str()) { probs.push(format!("{who}: \"{}\" is not an aggregation.", x.agg)); }
            if !FORMATS.contains(&x.format.as_str()) { x.format = "number".into(); }
            if !CALCS.contains(&x.calc.as_str()) { probs.push(format!("{who}: \"{}\" is not a calc.", x.calc)); }
            b.measures.push(x);
        }
    }
    if let Some(v) = j["window"].as_str() {
        if WINDOWS.contains(&v) { b.window = v.to_string(); } else { probs.push(format!("{who}: \"{v}\" is not a window.")); }
    }
    if let Some(v) = j["sort"].as_str() { if SORTS.contains(&v) { b.sort = v.to_string(); } }
    if let Some(v) = j["limit"].as_u64() { b.limit = v as u32; }
    if let Some(v) = j["others"].as_bool() { b.others = v; }
    if let Some(v) = j["mode"].as_str() { b.mode = if v == "rows" { "rows".into() } else { String::new() }; }
    if let Some(a) = j["columns"].as_array() {
        b.columns = a.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        for c in &b.columns {
            if !b.dataset.is_empty() && !field_ok(doc, &b.dataset, c) {
                probs.push(format!("{who}: source {} has no field \"{c}\".", b.dataset));
            }
        }
    }
}

/// Turns the model's answer into the page's widgets. `page`: the page being changed (kept
/// widgets are matched by id and only what the model gives changes); None builds a new page.
pub fn plan(doc: &Doc, page: Option<&Page>, ans: &Json) -> Result<Plan, Vec<String>> {
    let g = crate::vm::grid(doc);
    let mut probs = Vec::new();
    let list = ans["widgets"].as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        probs.push("The answer has no widgets.".into());
    }
    let mut out = Vec::new();
    let (mut added, mut changed) = (0, 0);
    for (i, j) in list.iter().enumerate() {
        let old = j["id"].as_str().and_then(|id| page.and_then(|p| p.widgets.iter().find(|w| w.id == id)));
        if j["id"].as_str().is_some() && old.is_none() {
            probs.push(format!("Widget {}: there is no widget \"{}\" on the page (leave the id out for a new one).", i + 1, j["id"].as_str().unwrap_or("")));
            continue;
        }
        let mut w = old.cloned().unwrap_or_default();
        let before = old.map(|o| serde_json::to_string(o).unwrap_or_default());
        if let Some(k) = j["kind"].as_str() {
            if !KINDS.iter().any(|(x, _)| *x == k) {
                probs.push(format!("Widget {}: \"{k}\" is not a widget type.", i + 1));
                continue;
            }
            if old.is_some_and(|o| o.kind != k) {
                // a new type starts from that type's own look
                w.variant.clear();
                w.style = json!({});
                w.opts = json!({});
            }
            w.kind = k.to_string();
        } else if old.is_none() {
            probs.push(format!("Widget {} has no kind.", i + 1));
            continue;
        }
        let who = format!("Widget {} ({}{})", i + 1, w.kind, j["title"].as_str().or(Some(&w.title)).filter(|t| !t.is_empty()).map(|t| format!(" “{t}”")).unwrap_or_default());
        for (k, v) in [("x", &mut w.x), ("y", &mut w.y), ("w", &mut w.w), ("h", &mut w.h)] {
            if let Some(n) = j[k].as_u64() { *v = n as u32; }
        }
        if old.is_none() && (j["w"].as_u64().is_none() || j["h"].as_u64().is_none()) {
            probs.push(format!("{who}: give x, y, w and h."));
        }
        w.w = w.w.max(1);
        w.h = w.h.max(1);
        if w.x + w.w > g.cols || w.y + w.h > g.rows {
            probs.push(format!("{who} at x {} y {} w {} h {} goes outside the {}×{} grid.", w.x, w.y, w.w, w.h, g.cols, g.rows));
        }
        if let Some(t) = j["title"].as_str() { w.title = t.to_string(); }
        if let Some(t) = j["subtitle"].as_str() { w.subtitle = t.to_string(); }
        if let Some(v) = j["variant"].as_str() {
            if v.is_empty() || doc.theme.styles.get(&w.kind).is_some_and(|l| l.iter().any(|x| x.id == v)) {
                if v != w.variant { w.style = json!({}); }
                w.variant = v.to_string();
            } else {
                probs.push(format!("{who}: \"{v}\" is not a design view of {} (views: {}).", w.kind,
                    doc.theme.styles.get(&w.kind).map(|l| l.iter().map(|x| x.id.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_else(|| "none".into())));
            }
        } else if old.is_none() {
            // a new widget takes the theme's default view of its kind
            w.variant = doc.theme.styles.get(&w.kind).and_then(|l| l.first()).map(|v| v.id.clone()).unwrap_or_default();
        }
        if let Some(t) = j["text"].as_str() {
            if !w.opts.is_object() { w.opts = json!({}); }
            w.opts["text"] = json!(t);
        }
        if let Some(o) = j["orientation"].as_str().filter(|_| w.kind == "bar") {
            if !w.opts.is_object() { w.opts = json!({}); }
            w.opts["orientation"] = json!(if o == "horizontal" { "horizontal" } else { "vertical" });
        }
        if DATA_KINDS.contains(&w.kind.as_str()) {
            let mut b = w.binding.clone().unwrap_or_default();
            if j["binding"].is_object() {
                read_binding(doc, &j["binding"], &mut b, &who, &mut probs);
            }
            if b.dataset.is_empty() || (b.measures.is_empty() && b.mode != "rows") {
                probs.push(format!("{who} needs a binding with a source and a measure."));
            }
            if w.kind == "kpi" { b.dims.clear(); }
            w.binding = Some(b);
        } else {
            w.binding = None;
        }
        if w.kind == "heading" && w.opts.get("text").is_none() {
            w.opts = json!({"text": w.title});
        }
        match before {
            Some(b) if b != serde_json::to_string(&w).unwrap_or_default() => changed += 1,
            Some(_) => {}
            None => added += 1,
        }
        out.push(w);
    }
    // overlaps the fit engine would have to push around
    for i in 0..out.len() {
        for k in i + 1..out.len() {
            let (a, b) = (&out[i], &out[k]);
            if a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h {
                probs.push(format!("“{}” ({}) and “{}” ({}) overlap.", if a.title.is_empty() { &a.kind } else { &a.title }, a.kind, if b.title.is_empty() { &b.kind } else { &b.title }, b.kind));
            }
        }
    }
    if !probs.is_empty() {
        return Err(probs);
    }
    let removed = page.map(|p| p.widgets.iter().filter(|w| !out.iter().any(|x| x.id == w.id)).count()).unwrap_or(0);
    let title = ans["title"].as_str().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).or_else(|| page.map(|p| p.title.clone())).unwrap_or_default();
    Ok(Plan { title, widgets: out, added, changed, removed })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Doc {
        let mut d = crate::sample::template();
        d.page.cols = 12;
        d.page.rows = 12;
        // the fields a loaded source would list
        d.datasets[0].fields = vec![
            crate::model::Field { name: "region".into(), kind: "text".into() },
            crate::model::Field { name: "revenue".into(), kind: "number".into() },
        ];
        d
    }

    #[test]
    fn context_lists_structure_only() {
        let d = doc();
        let c = context(&d, Some(&d.pages[0]));
        let ds = &d.datasets[0];
        assert!(c.contains(&ds.id) && c.contains(&ds.fields[0].name));
        assert!(c.contains("12 columns × 12 rows"));
        assert!(c.contains("THE CURRENT PAGE"));
    }

    #[test]
    fn plan_checks_names_and_room() {
        let d = doc();
        let ds = &d.datasets[0];
        let num = ds.fields.iter().find(|f| f.kind == "number").unwrap().name.clone();
        let ok = json!({"title": "T", "widgets": [
            {"kind": "heading", "x": 0, "y": 0, "w": 12, "h": 1, "text": "Overview"},
            {"kind": "kpi", "x": 0, "y": 1, "w": 4, "h": 3, "title": "Total", "binding": {"dataset": ds.name, "measures": [{"field": num, "agg": "sum"}]}},
        ]});
        let p = plan(&d, None, &ok).unwrap();
        assert_eq!(p.added, 2);
        assert_eq!(p.widgets[1].binding.as_ref().unwrap().dataset, ds.id, "a source named by its name resolves to its id");
        let bad = json!({"widgets": [
            {"kind": "kpi", "x": 0, "y": 0, "w": 4, "h": 3, "binding": {"dataset": ds.id, "measures": [{"field": "nope", "agg": "sum"}]}},
            {"kind": "bar", "x": 2, "y": 1, "w": 12, "h": 4, "binding": {"dataset": ds.id, "measures": [{"field": num}]}},
        ]});
        let e = plan(&d, None, &bad).unwrap_err().join("\n");
        assert!(e.contains("no field \"nope\""), "{e}");
        assert!(e.contains("outside the 12×12 grid"), "{e}");
        assert!(e.contains("overlap"), "{e}");
    }

    #[test]
    fn changing_a_page_keeps_what_the_model_leaves_alone() {
        let d = doc();
        let p = d.pages.iter().find(|p| p.widgets.len() >= 2).unwrap();
        let (a, b) = (&p.widgets[0], &p.widgets[1]);
        let ans = json!({"widgets": [{"id": a.id, "title": "Renamed"}]});
        let r = plan(&d, Some(p), &ans);
        // only overlaps could fail here: the kept widget is where it was
        if let Ok(r) = r {
            assert_eq!(r.widgets.len(), 1);
            assert_eq!(r.widgets[0].title, "Renamed");
            assert_eq!(r.widgets[0].opts, a.opts, "options it didn't mention stay");
            assert_eq!(r.changed, 1);
            assert_eq!(r.removed, p.widgets.len() - 1);
        }
        let _ = b;
    }
}
