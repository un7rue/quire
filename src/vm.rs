//! The view model: page geometry, query results, wrapped text and table
//! metrics for every widget, plus each page's fit analysis. Rust decides all
//! typography-dependent sizes so the screen and every export agree.

use crate::layout::{self, Grid, Item, PageCtx};
use crate::model::{Doc, DocKind, FontSpec, Frame, Page, Widget};
use crate::period;
use crate::text::{self, Block, Font};
use serde::Serialize;
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, HashMap};

pub const PAD: f64 = 10.0;
const HEAD_GAP: f64 = 7.0;

/// The space between a widget's title (and subtitle) and what it shows: the widget's own
/// (style.titleGap, in points), or the usual.
fn title_gap(w: &Widget) -> f64 {
    w.style_f64("titleGap").map(|v| v.clamp(0.0, 40.0)).unwrap_or(HEAD_GAP - 3.0)
}

/// Every text role a report uses.
pub const ROLES: [&str; 15] =
    ["heading", "heading_sub", "eyebrow", "title", "subtitle", "body", "caption", "kpi", "table_head", "table_cell", "chart", "footer", "axis", "data_label", "legend"];

/// A chart's own parts' text: each starts as the "chart" role at the size the drawing has always
/// given it (axis labels a little smaller, data labels and the legend a little larger), in its grey;
/// the theme's and the widget's settings for the part go on top.
fn chart_part(role: &str) -> Option<f64> {
    match role {
        "axis" => Some(-0.5),
        "data_label" | "legend" => Some(0.5),
        _ => None,
    }
}

/// Built-in defaults: (size, weight, muted colour?). A KPI size of 0 means "fit the card".
fn role_default(role: &str) -> (f64, u16, bool) {
    match role {
        "heading" => (20.0, 700, false),
        "heading_sub" => (10.0, 400, true),
        "eyebrow" => (7.5, 700, false),
        "title" => (9.5, 600, false),
        "subtitle" => (8.0, 400, true),
        "body" => (9.5, 400, false),
        "caption" => (7.5, 600, true),
        "kpi" => (0.0, 600, false),
        "table_head" => (8.0, 600, true),
        "table_cell" => (8.0, 400, false),
        "chart" => (7.0, 400, true),
        "footer" => (7.0, 400, true),
        _ => (9.5, 400, false),
    }
}

pub fn apply_spec(f: &mut Font, s: &FontSpec) {
    if let Some(v) = s.family.as_ref().filter(|v| !v.is_empty()) {
        f.family = v.clone();
    }
    if let Some(v) = s.size.filter(|v| *v > 0.0) {
        f.size = v.clamp(4.0, 120.0);
    }
    if let Some(v) = s.weight {
        f.weight = v.clamp(100, 900);
    }
    if let Some(v) = s.italic {
        f.italic = v;
    }
    if let Some(v) = s.color.as_ref().filter(|v| !v.is_empty()) {
        f.color = v.clone();
    }
    if let Some(v) = s.upper {
        f.upper = v;
    }
    if let Some(v) = s.tracking {
        f.tracking = v.clamp(-0.1, 0.5);
    }
}

/// Resolves a role: built-in default → kind tweaks → theme → widget.
pub fn font(doc: &Doc, w: Option<&Widget>, role: &str) -> Font {
    let th = &doc.theme;
    if let Some(delta) = chart_part(role) {
        let mut f = font(doc, w, "chart");
        f.size += delta;
        f.color = th.muted.clone();
        // sizes set for the part scale as every other text does
        let k = th.font_scale.clamp(0.5, 2.5) * w.and_then(|w| w.style_f64("textScale")).map(|k| k.clamp(0.6, 1.6)).unwrap_or(1.0);
        let own = w.and_then(|w| w.style.get("fonts").and_then(|f| f.get(role))).and_then(|s| serde_json::from_value::<FontSpec>(s.clone()).ok());
        for s in th.fonts.get(role).into_iter().chain(own.as_ref()) {
            let mut s = s.clone();
            s.size = s.size.map(|v| v * k);
            apply_spec(&mut f, &s);
        }
        if f.family == "@display" {
            f.family = th.fonts.get("heading").and_then(|s| s.family.clone()).unwrap_or_else(|| th.font_family.clone());
        } else if f.family == "@ui" {
            f.family = th.font_family.clone();
        }
        f.color = th.color(&f.color);
        return f;
    }
    let (size, weight, muted) = role_default(role);
    let mut f = Font {
        family: if th.font_family.is_empty() { crate::fonts::DEFAULT_FAMILY.into() } else { th.font_family.clone() },
        size,
        weight,
        italic: false,
        color: if muted { th.muted.clone() } else { th.ink.clone() },
        upper: false,
        tracking: 0.0,
    };
    if role == "eyebrow" {
        // the small capitals line above a heading, in the accent colour
        f.color = th.accent.clone();
        f.upper = true;
        f.tracking = 0.14;
    }
    if let Some(w) = w {
        if w.kind == "comment" && role == "body" {
            f.size = 9.0;
        }
        if w.kind == "table" && (role == "table_cell" || role == "table_head") {
            match w.opt_str("density") {
                Some("compact") => f.size = 7.5,
                Some("comfortable") => f.size = 8.5,
                _ => {}
            }
        }
    }
    if let Some(s) = th.fonts.get(role) {
        apply_spec(&mut f, s);
    }
    if let Some(w) = w {
        // older style keys still apply
        let main = matches!((w.kind.as_str(), role), ("heading", "heading") | ("text", "body") | ("comment", "body"));
        if main {
            if let Some(v) = w.style_f64("size") {
                f.size = v;
            }
            if let Some(v) = w.style.get("weight").and_then(|v| v.as_u64()) {
                f.weight = v as u16;
            }
            // a note's colour is its bar and its avatar; its words keep their own
            if let Some(v) = w.style.get("color").and_then(|v| v.as_str()).filter(|_| w.kind != "comment") {
                f.color = v.into();
            }
        }
        if role.starts_with("table_") {
            if let Some(v) = w.opts.pointer("/table/size").and_then(|v| v.as_f64()) {
                f.size = v.clamp(5.0, 24.0);
            }
        }
        // a title as the theme sets it ("strong") leaves out a look's own title font
        let title_style = w.style.get("titleStyle").and_then(|v| v.as_str());
        if let Some(spec) = w.style.get("fonts").and_then(|f| f.get(role)).filter(|_| !(role == "title" && title_style == Some("strong"))) {
            if let Ok(s) = serde_json::from_value::<FontSpec>(spec.clone()) {
                apply_spec(&mut f, &s);
            }
        }
        if role == "title" && title_style == Some("dim") {
            // a quiet title: smaller, in sentence case, in the body's grey
            let base = th.fonts.get("title").and_then(|s| s.size).unwrap_or(f.size);
            f.size = (base * 0.82 * 10.0).round() / 10.0;
            f.weight = 600;
            f.upper = false;
            f.tracking = 0.0;
            f.color = th.token("body").unwrap_or_else(|| th.muted.clone());
        }
    }
    // a widget's text size against the theme (smaller to fit, larger to stand out)
    if let Some(k) = w.and_then(|w| w.style_f64("textScale")) {
        f.size *= k.clamp(0.6, 1.6);
    }
    f.size *= th.font_scale.clamp(0.5, 2.5);
    // "@display" and "@ui": the theme's heading and body faces, so a widget can follow any theme
    if f.family == "@display" {
        f.family = th.fonts.get("heading").and_then(|s| s.family.clone()).unwrap_or_else(|| th.font_family.clone());
    } else if f.family == "@ui" {
        f.family = th.font_family.clone();
    }
    f.color = th.color(&f.color);
    f
}

pub fn fonts_for(doc: &Doc, w: Option<&Widget>) -> BTreeMap<&'static str, Font> {
    ROLES.iter().map(|r| (*r, font(doc, w, r))).collect()
}

#[derive(Serialize)]
pub struct TextVm {
    /// Block origin, absolute in page points.
    pub x: f64,
    pub y: f64,
    pub block: Block,
    pub color: Option<String>,
}

/// A decoration Rust places exactly: a filled shape, a rule or a single line of text.
#[derive(Serialize, Clone, Debug)]
pub struct Deco {
    /// "rect", "line" or "text"
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fill: String,
    pub radius: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Text font; the text's baseline is `y`, anchored by `anchor`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<Font>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub anchor: String,
}

impl Deco {
    fn rect(x: f64, y: f64, w: f64, h: f64, fill: &str, radius: f64) -> Deco {
        Deco { kind: "rect", x, y, w, h, fill: fill.into(), radius, text: String::new(), font: None, anchor: String::new() }
    }
    fn line(x: f64, y: f64, w: f64, h: f64, color: &str) -> Deco {
        Deco { kind: "line", x, y, w, h, fill: color.into(), radius: 0.0, text: String::new(), font: None, anchor: String::new() }
    }
    fn text(x: f64, y: f64, text: String, f: &Font, anchor: &str) -> Deco {
        let w = text::width_f(&text, f);
        Deco { kind: "text", x, y, w, h: f.size, fill: String::new(), radius: 0.0, text, font: Some(f.clone()), anchor: anchor.into() }
    }
}

/// The pages a heading's navigation lists: the page's group, else its section.
pub fn nav_pages(doc: &Doc, w: &Widget) -> Vec<(String, bool)> {
    let Some(pi) = doc.pages.iter().position(|p| p.widgets.iter().any(|x| x.id == w.id)) else { return Vec::new() };
    let me = &doc.pages[pi];
    let same = |p: &Page| if !me.group.is_empty() { p.group == me.group } else { !me.section.is_empty() && p.section == me.section };
    doc.pages
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.auto && same(p) && (me.group.is_empty() || !p.label.is_empty()) && page_shown(doc, p))
        .map(|(i, p)| (with_row(&p.row, || expand(if p.label.is_empty() { &p.title } else { &p.label }, doc)), i == pi))
        .collect()
}

#[derive(Serialize)]
pub struct TableVm {
    pub row_h: f64,
    pub head_h: f64,
    /// Height of the header-groups row (0 when there are none).
    pub group_h: f64,
    pub font: f64,
    /// First data row this piece shows (continuation pieces start later).
    pub offset: u32,
    /// Data rows that fit.
    pub visible: u32,
    /// Subheaders repeated on top of this piece (row indexes): the groups it starts inside.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub carry: Vec<u32>,
    /// All data rows.
    pub rows: u32,
    pub totals: bool,
    /// More rows follow on a continuation piece.
    pub continued: bool,
    /// 0 for the original table, 1… for continuations.
    pub part: u32,
    /// Height of each visible row (wrapped cells make rows taller).
    pub heights: Vec<f64>,
    /// Wrapped lines for wrapping columns: key → per visible row.
    pub wrapped: BTreeMap<String, Vec<Vec<String>>>,
    /// Second lines under cells were dropped so every row fits ("fill" tables).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_sub: bool,
}

/// Where a piece of a long table sits in its chain.
#[derive(Clone, Copy, Debug, Default)]
pub struct Chain {
    pub offset: u32,
    pub last: bool,
    pub part: u32,
}

/// Lines a cell needs when its column wraps.
fn wrap_cell(text: &str, width: f64, f: &Font, max_lines: usize) -> Vec<String> {
    let mut lines = text::wrap(text, width, f);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(l) = lines.last_mut() {
            *l = text::ellipsize(&format!("{l}…"), width, f);
        }
    }
    lines
}

/// Rows, heights and wrapping for one table piece.
fn table_vm(w: &Widget, fonts: &BTreeMap<&'static str, Font>, frame: &Frame, body: &Frame, data: Option<&Json>, chain: Chain) -> (TableVm, f64) {
    let factor = match w.opt_str("density") {
        Some("compact") => 1.87,
        Some("comfortable") => 2.35,
        _ => 2.125,
    };
    let cf = &fonts["table_cell"];
    let cell = cf.size;
    let head = fonts["table_head"].size;
    let ts = w.opts.get("table");
    let fixed = |k: &str| ts.and_then(|t| t.get(k)).and_then(|v| v.as_f64()).filter(|v| *v >= 6.0);
    let row_h = fixed("row_h").unwrap_or(cell * factor).max(cell * 1.2);
    let hide_head = ts.and_then(|t| t.get("hideHead")).and_then(|v| v.as_bool()).unwrap_or(false);
    let head_h = if hide_head { 0.0 } else { fixed("head_h").unwrap_or((head * factor).max(row_h) + 3.0).max(head * 1.2) };
    let col_style = |key: &str, base: Option<&str>| -> Option<&Json> {
        let cols = ts.and_then(|t| t.get("cols"))?;
        cols.get(key).or_else(|| base.and_then(|b| cols.get(b)))
    };
    let empty = vec![];
    let columns = data.and_then(|d| d.get("columns")).and_then(|c| c.as_array()).unwrap_or(&empty);
    let rows_json = data.and_then(|d| d.get("rows")).and_then(|r| r.as_array()).unwrap_or(&empty);
    let has_groups = columns.iter().any(|c| {
        let key = c["key"].as_str().unwrap_or("");
        let style = col_style(key, c["base"].as_str());
        let hidden = style.and_then(|s| s.get("hidden")).and_then(|v| v.as_bool()).unwrap_or(false);
        !hidden && (c["group"].as_str().is_some_and(|g| !g.is_empty()) || style.and_then(|s| s.get("group")).and_then(|v| v.as_str()).is_some_and(|g| !g.is_empty()))
    });
    let group_h = if has_groups { head * 1.9 } else { 0.0 };
    // wrapping columns: width is a share of the table, measured with the cell font
    let line_h = cell * 1.3;
    let mut wraps: Vec<(usize, String, f64, Font, usize)> = Vec::new();
    for (j, c) in columns.iter().enumerate() {
        let key = c["key"].as_str().unwrap_or("").to_string();
        let Some(st) = col_style(&key, c["base"].as_str()) else { continue };
        if !st.get("wrap").and_then(|v| v.as_bool()).unwrap_or(false) || st.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let pct = st.get("width").and_then(|v| v.as_f64()).filter(|v| *v > 0.0).unwrap_or(30.0).clamp(3.0, 90.0);
        let mut f = cf.clone();
        if let Some(fam) = st.get("family").and_then(|v| v.as_str()) {
            f.family = fam.into();
        }
        if let Some(i) = st.get("italic").and_then(|v| v.as_bool()) {
            f.italic = i;
        }
        if st.get("bold").and_then(|v| v.as_bool()).unwrap_or(false) {
            f.weight = f.weight.max(600);
        }
        let max_lines = st.get("maxLines").and_then(|v| v.as_u64()).unwrap_or(4).clamp(1, 12) as usize;
        // an icon before the text takes its share of the width (the drawing puts it beside the lines)
        // (as the drawing sizes it: the chip at 1.9 × the text size, scaled, and a gap of 0.7)
        let icon_w = if st.get("icon").is_some_and(|i| i.is_object()) { cell * (1.9 * st.pointer("/icon/size").and_then(|v| v.as_f64()).unwrap_or(1.0).clamp(0.3, 2.0) + 0.7) } else { 0.0 };
        wraps.push((j, key, body.w * pct / 100.0 - 10.0 - icon_w, f, max_lines));
    }
    let cell_text = |v: &Json| match v {
        Json::String(s) => s.clone(),
        Json::Null => String::new(),
        other => other.to_string(),
    };
    let total = rows_json.len();
    // a second line under a cell (e.g. ISIN under the name) makes rows taller
    let tree_sub = ts.and_then(|t| t.pointer("/cols/__tree/sub")).and_then(|v| v.as_str()).is_some_and(|v| !v.is_empty())
        && data.and_then(|d| d.get("row_meta")).is_some();
    let sub_on = tree_sub || columns.iter().any(|c| {
        let st = col_style(c["key"].as_str().unwrap_or(""), c["base"].as_str());
        st.and_then(|s| s.get("sub")).and_then(|v| v.as_str()).is_some_and(|v| !v.is_empty())
            && !st.and_then(|s| s.get("hidden")).and_then(|v| v.as_bool()).unwrap_or(false)
    });
    let fill = ts.and_then(|t| t.get("fill")).and_then(|v| v.as_bool()).unwrap_or(false);
    let sub_h = if sub_on { row_h.max(cell * 2.75) } else { row_h };
    let height_of = |r: usize, base_h: f64| -> (f64, Vec<(String, Vec<String>)>) {
        let mut h = base_h;
        let mut cells = Vec::new();
        for (j, key, width, f, max) in &wraps {
            let text = rows_json[r].get(*j).map(cell_text).unwrap_or_default();
            let lines = if text.is_empty() { vec![String::new()] } else { wrap_cell(&text, width.max(10.0), f, *max) };
            h = h.max(lines.len() as f64 * line_h + (row_h - line_h));
            cells.push((key.clone(), lines));
        }
        (h, cells)
    };
    let totals_on = w.opts.get("totals").and_then(|v| v.as_bool()).unwrap_or(true);
    let totals = totals_on && chain.last;
    let start = (chain.offset as usize).min(total);
    // grouped rows: a piece that starts inside a group repeats that group's subheader(s) on top
    let meta: Vec<(bool, u64)> = data
        .and_then(|d| d.get("row_meta"))
        .and_then(|m| m.as_array())
        .map(|m| m.iter().map(|x| (x["kind"].as_str() == Some("group"), x["level"].as_u64().unwrap_or(0))).collect())
        .unwrap_or_default();
    let carry: Vec<u32> = carried_groups(&meta, start).into_iter().map(|k| k as u32).collect();
    let carried = carry.len();
    let fit = |base_h: f64| {
        let avail = body.h - head_h - group_h - if totals { row_h } else { 0.0 } - carried as f64 * base_h;
        let mut heights = Vec::new();
        let mut wrapped: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
        let mut used = 0.0;
        let mut all_h = 0.0;
        for r in start..total {
            let (h, cells) = height_of(r, base_h);
            all_h += h;
            if used + h <= avail + 1e-6 && heights.len() == r - start {
                used += h;
                heights.push(h);
                for (k, lines) in cells {
                    wrapped.entry(k).or_default().push(lines);
                }
            }
        }
        (avail, heights, wrapped, used, all_h)
    };
    let (mut avail, mut heights, mut wrapped, mut used, mut all_h) = fit(sub_h);
    // a "fill" table would rather lose its second lines than hide rows
    let mut no_sub = false;
    let mut base_h = sub_h;
    if fill && sub_on && !tree_sub && start == 0 && heights.len() < total {
        let tight = fit(row_h);
        if tight.1.len() > heights.len() {
            (avail, heights, wrapped, used, all_h) = tight;
            no_sub = true;
            base_h = row_h;
        }
    }
    // …and then tighter rows, down to one and a half lines of text (a table that runs on
    // to more pages keeps the row height it was given and continues instead)
    let floor = cell * 1.55;
    // continuing is on unless the table turns it off (as in `paginated`)
    let paginates = ts.and_then(|t| t.get("paginate")).and_then(|v| v.as_bool()).unwrap_or(true) || !chain.last || chain.part > 0;
    if fill && !paginates && start == 0 && heights.len() < total && base_h > floor {
        let mut lo = floor;
        let mut hi = base_h;
        for _ in 0..12 {
            let mid = (lo + hi) / 2.0;
            if fit(mid).1.len() == total { lo = mid } else { hi = mid }
        }
        let tight = fit(lo);
        if tight.1.len() > heights.len() {
            (avail, heights, wrapped, used, all_h) = tight;
            base_h = lo;
        }
    }
    // keep with next: a piece that stops before the end never ends on a subheader — it moves on with its rows
    if meta.len() == total {
        while !heights.is_empty() && start + heights.len() < total && meta[start + heights.len() - 1].0 {
            heights.pop();
            for v in wrapped.values_mut() {
                v.pop();
            }
        }
    }
    // "fill": a table whose rows all fit spreads them over its height (up to twice as tall),
    // so a short list doesn't leave half its place empty
    if fill && start == 0 && heights.len() == total && total > 0 && used < avail {
        let grow = ((avail - used) / total as f64).min(base_h);
        for h in heights.iter_mut() {
            *h += grow;
        }
    }
    let visible = heights.len() as u32;
    let continued = !chain.last && start + (visible as usize) < total;
    let need = head_h + group_h + all_h + if totals_on { row_h } else { 0.0 } + (frame.h - body.h);
    (
        TableVm {
            row_h,
            head_h,
            group_h,
            font: cell,
            offset: start as u32,
            carry,
            visible,
            rows: total as u32,
            totals,
            continued,
            part: chain.part,
            heights,
            wrapped,
            no_sub,
        },
        need,
    )
}

/// Whether a table carries its overflow onto new pages (on unless turned off).
pub fn paginated(theme: &crate::model::Theme, w: &Widget) -> bool {
    let w = crate::model::effective(theme, w);
    w.kind == "table" && continues(&w).is_none() && w.opts.pointer("/table/paginate").and_then(|v| v.as_bool()).unwrap_or(true)
}

/// A widget's height on the page: a table with linked widgets takes their rows while it continues.
pub fn eff_h(doc: &Doc, w: &Widget) -> u32 {
    w.h + doc.grow.get(&w.id).copied().unwrap_or(0)
}

/// The table a widget follows (its linked widgets go after the table's last rows).
pub fn follows(w: &Widget) -> Option<&str> {
    w.opts.get("follows").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
}

/// The id of the table a continuation piece belongs to.
pub fn continues(w: &Widget) -> Option<&str> {
    w.opts.pointer("/table/continues").and_then(|v| v.as_str()).filter(|s| !s.is_empty())
}

/// Offsets for every piece of every paginated table.
pub fn chains(doc: &Doc, results: &HashMap<String, Json>) -> HashMap<String, Chain> {
    let mut out = HashMap::new();
    for p in &doc.pages {
        for src in p.widgets.iter().filter(|w| paginated(&doc.theme, w) && continues(w).is_none()) {
            let pieces: Vec<&Widget> = std::iter::once(src)
                .chain(doc.pages.iter().flat_map(|q| q.widgets.iter()).filter(|w| continues(w) == Some(src.id.as_str())))
                .collect();
            let data = results.get(&src.id);
            let rows = data.and_then(|d| d.get("rows")).and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0) as u32;
            let mut offset = 0u32;
            let mut done = false;
            for (i, piece) in pieces.iter().enumerate() {
                // the rows ran out on an earlier piece: this one is empty (its page is left out)
                if done {
                    out.insert(piece.id.clone(), Chain { offset: rows.max(offset), last: true, part: i as u32 });
                    continue;
                }
                let frame = doc.page.frame(piece.x, piece.y, piece.w, eff_h(doc, piece));
                // the piece where everything left fits is the last: it shows the totals
                let as_last = Chain { offset, last: true, part: i as u32 };
                let fits = i + 1 == pieces.len() || widget_vm_chain(doc, piece, frame, data.cloned(), as_last).table.is_none_or(|t| offset + t.visible >= rows);
                if fits {
                    out.insert(piece.id.clone(), as_last);
                    offset = rows.max(offset);
                    done = true;
                    continue;
                }
                let chain = Chain { offset, last: false, part: i as u32 };
                out.insert(piece.id.clone(), chain);
                let vm = widget_vm_chain(doc, piece, frame, data.cloned(), chain);
                offset += vm.table.map(|t| t.visible.max(1)).unwrap_or(1);
            }
        }
    }
    out
}

/// The group subheaders a piece starting at `start` repeats on top: the
/// groups it starts inside, outermost first (indexes into the rows).
pub fn carried_groups(meta: &[(bool, u64)], start: usize) -> Vec<usize> {
    let Some(&(is_group, level)) = meta.get(start) else { return Vec::new() };
    if start == 0 {
        return Vec::new();
    }
    // a piece that starts on a subheader shows it anyway; inside it, the levels above it
    let _ = is_group;
    let mut want = level;
    let mut out = Vec::new();
    for k in (0..start).rev() {
        if want == 0 {
            break;
        }
        let (g, l) = meta[k];
        if g && l < want {
            out.push(k);
            want = l;
        }
    }
    out.reverse();
    out
}

/// How many pieces a paginated table needs, given its current pieces and
/// the frame a new continuation would get.
pub fn pieces_needed(doc: &Doc, pieces: &[&Widget], extra: &Widget, data: Option<&Json>) -> usize {
    let rows = data.and_then(|d| d.get("rows")).and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0) as u32;
    let mut offset = 0u32;
    for i in 0..60 {
        let w = pieces.get(i).copied().unwrap_or(extra);
        let frame = doc.page.frame(w.x, w.y, w.w, eff_h(doc, w));
        // as the last piece (totals shown) does everything left fit?
        let last = widget_vm_chain(doc, w, frame, data.cloned(), Chain { offset, last: true, part: i as u32 });
        let t = last.table.unwrap();
        if offset + t.visible >= rows {
            return i + 1;
        }
        let mid = widget_vm_chain(doc, w, frame, data.cloned(), Chain { offset, last: false, part: i as u32 });
        offset += mid.table.map(|t| t.visible.max(1)).unwrap_or(1);
    }
    60
}

#[derive(Serialize)]
pub struct WidgetVm {
    pub id: String,
    pub frame: Frame,
    pub body: Frame,
    pub card: bool,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub title_y: f64,
    pub sub_y: f64,
    pub data: Option<Json>,
    pub texts: Vec<TextVm>,
    pub table: Option<TableVm>,
    /// Rows the content needs, when that's known.
    pub need_h: Option<u32>,
    /// Resolved fonts for every role.
    pub fonts: BTreeMap<&'static str, Font>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toc: Option<Vec<TocEntry>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deco: Vec<Deco>,
    /// Options and style with the widget's design view applied (when it has one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eff: Option<Json>,
    /// Visualization texts with placeholders filled (key → text).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub viz_text: BTreeMap<String, String>,
    /// A linked widget shown after its table's last rows instead of here.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub moved: bool,
    /// A composite's parts, each laid out in its place inside the card (as `widget.parts`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<WidgetVm>,
}

#[derive(Serialize)]
pub struct PageVm {
    pub id: String,
    /// Left out for the current data (its show-if condition is empty or 0): not exported, not numbered.
    pub hidden: bool,
    /// Its number among the shown pages (0 when hidden).
    pub number: usize,
    /// The title with its placeholders filled in.
    pub title: String,
    /// A repeated page: which copy (0 = the designed page), how many, and what it is for.
    pub copy: usize,
    pub copies: usize,
    pub copy_label: String,
    /// The page's main heading (title, subtitle), for the contents helper.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<(String, String)>,
    pub widgets: Vec<WidgetVm>,
    pub issues: Vec<layout::Issue>,
    /// Its paper when it isn't the theme's page colour (the cover's, the closing page's).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
}

/// The theme's colour blocks (tokens block1…block4), for views set to `fillCycle`.
fn blocks(doc: &Doc) -> Vec<String> {
    (1..=4).filter_map(|i| doc.theme.token(&format!("block{i}"))).collect()
}

/// A widget of a `fillCycle` view as it shows on its page: the `k`-th block colour as its card,
/// its text in the ink — or the page colour where the ink wouldn't read on that block.
fn on_block(doc: &Doc, w: &Widget, fill: &str) -> Widget {
    let mut c = w.clone();
    if !c.style.is_object() {
        c.style = serde_json::json!({});
    }
    let ink = doc.theme.ink.clone();
    let text = if crate::ai_restyle::contrast(&ink, fill) >= 4.5 { ink } else { doc.theme.page_bg.clone() };
    // figures in the block's text colour (not green / red by sign, not a tinted card of their own)
    if !c.opts.is_object() {
        c.opts = serde_json::json!({});
    }
    if !c.opts["viz"].is_object() {
        c.opts["viz"] = serde_json::json!({});
    }
    c.opts["viz"]["valueColor"] = serde_json::json!("font");
    c.opts["viz"]["tone"] = serde_json::json!("none");
    c.style["card"] = serde_json::json!(true);
    c.style["background"] = serde_json::json!(fill);
    // no frame on a block (a theme with outlined cards): its edge in its own colour
    c.style["border"] = serde_json::json!(fill);
    for role in ["title", "subtitle", "kpi", "caption", "body", "heading", "eyebrow", "chart", "table_head", "table_cell", "axis", "data_label", "legend"] {
        if !c.style["fonts"].is_object() {
            c.style["fonts"] = serde_json::json!({});
        }
        if !c.style["fonts"][role].is_object() {
            c.style["fonts"][role] = serde_json::json!({});
        }
        c.style["fonts"][role]["color"] = serde_json::json!(text);
    }
    c
}

thread_local! {
    static LOOKUPS: std::cell::RefCell<HashMap<String, String>> = std::cell::RefCell::new(HashMap::new());
    /// The row of the repeated page being built (`{{row.column}}`).
    static ROW: std::cell::RefCell<std::collections::BTreeMap<String, String>> = std::cell::RefCell::new(Default::default());
}

/// Runs `f` with a repeated page's row as the current one.
pub fn with_row<T>(row: &std::collections::BTreeMap<String, String>, f: impl FnOnce() -> T) -> T {
    let prev = ROW.with(|r| std::mem::replace(&mut *r.borrow_mut(), row.clone()));
    let out = f();
    ROW.with(|r| *r.borrow_mut() = prev);
    out
}

/// Fills `{{row.column}}` with the row's values ("" for a column it doesn't have).
/// `json`: the text is JSON, so values are escaped for a JSON string.
pub fn fill_row(s: &str, row: &std::collections::BTreeMap<String, String>, json: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("{{row.") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 6..];
        match after.find("}}") {
            Some(j) => {
                let v = row.get(after[..j].trim()).map(|v| v.as_str()).unwrap_or("");
                if json {
                    let q = serde_json::to_string(v).unwrap_or_default();
                    out.push_str(&q[1..q.len() - 1]);
                } else {
                    out.push_str(v);
                }
                rest = &after[j + 2..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether a page shows for the current data: its condition, filled in, is
/// neither empty nor 0 / — / false.
pub fn page_shown(doc: &Doc, p: &Page) -> bool {
    if p.piece_empty {
        return false;
    }
    // a repeated page whose source has no matching rows has nothing to show
    if !p.repeat.is_off() && p.row.is_empty() {
        return false;
    }
    with_row(&p.row, || page_shown_now(doc, p))
}

fn page_shown_now(doc: &Doc, p: &Page) -> bool {
    // a continuation page follows the page its table starts on
    if p.auto {
        if let Some(src) = p.widgets.iter().find_map(|w| continues(w)) {
            if let Some(sp) = doc.pages.iter().find(|q| !q.auto && q.widgets.iter().any(|w| w.id == src)) {
                return page_shown(doc, sp);
            }
        }
    }
    let c = p.show_if.trim();
    if c.is_empty() {
        return true;
    }
    let v = expand(c, doc);
    let v = v.trim();
    !(v.is_empty() || v == "—" || v == "-" || v.eq_ignore_ascii_case("false") || v.parse::<f64>().is_ok_and(|n| n == 0.0) || v.starts_with("[") && v.ends_with("?]"))
}

/// Each page's number among the shown pages (None when hidden).
pub fn page_numbers(doc: &Doc) -> Vec<Option<usize>> {
    let mut n = 0;
    doc.pages.iter().map(|p| if page_shown(doc, p) { n += 1; Some(n) } else { None }).collect()
}

/// Values for `{{data:Source.field}}` placeholders, resolved by the engine.
pub fn set_lookups(m: HashMap<String, String>) {
    LOOKUPS.with(|l| *l.borrow_mut() = m);
}

/// Every `{{sql:…}}` query in a text.
pub fn sql_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("{{sql:") {
        let after = &rest[i + 6..];
        match after.find("}}") {
            Some(j) => {
                out.push(after[..j].trim().to_string());
                rest = &after[j + 2..];
            }
            None => break,
        }
    }
    out
}

/// A header or footer, its texts expanded (page numbers are filled per page by the renderer).
pub fn band(doc: &Doc, b: &crate::model::Band) -> serde_json::Value {
    let base = font(doc, None, "footer");
    let slot = |s: &crate::model::Slot| {
        let mut f = base.clone();
        apply_spec(&mut f, &s.font);
        f.color = doc.theme.color(&f.color);
        json!({"text": expand(&s.text, doc), "image": s.image, "image_h": s.image_h, "image_ratio": s.image_ratio, "font": f})
    };
    json!({
        "show": b.show, "height": b.height, "fill": doc.theme.color(&b.fill), "line": b.line, "line_color": doc.theme.color(&b.line_color), "tabs": b.tabs,
        "left": slot(&b.left), "center": slot(&b.center), "right": slot(&b.right),
    })
}

/// Every `{{data:…}}` token in a text.
pub fn data_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("{{data:") {
        let after = &rest[i + 7..];
        match after.find("}}") {
            Some(j) => {
                out.push(after[..j].trim().to_string());
                rest = &after[j + 2..];
            }
            None => break,
        }
    }
    out
}

/// Replaces {{period}}, {{start}}, {{title}}, {{data:Source.field}}… in text.
/// Initials of a name, for an avatar ("Warren Buffington" → "WB").
fn initials(name: &str) -> String {
    name.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(|c| c.to_uppercase()).collect()
}

/// Signed numbers get an arrow and their colour: "+6,48%" → green ▲ +6,48%,
/// "−2,1%" → red ▼ −2,1%. A sign counts at the start of a word, before a digit;
/// numbers already styled (inside a span) stay as they are.
pub fn auto_arrows(s: &str, good: &str, bad: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 32);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // the start of a word; bold or italic marks before it count as its start
        let mut k = i;
        while k > 0 && chars[k - 1] == '*' {
            k -= 1;
        }
        let starts = k == 0 || chars[k - 1].is_whitespace() || chars[k - 1] == '(';
        if matches!(c, '+' | '-' | '−') && starts && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()) && !after_icon(&out) {
            let mut j = i + 1;
            while j < chars.len() {
                let d = chars[j];
                let sep = matches!(d, ',' | '.' | '\u{a0}' | '\u{202f}' | ' ') && chars.get(j + 1).is_some_and(|n| n.is_ascii_digit());
                if d.is_ascii_digit() || sep {
                    j += 1;
                } else {
                    break;
                }
            }
            if chars.get(j) == Some(&'%') {
                j += 1;
            }
            let num: String = chars[i..j].iter().collect();
            let (icon, col) = if c == '+' { ("up", good) } else { ("down", bad) };
            out.push_str(&format!("{{#{}|{{!{icon}}} {num}}}", col.trim_start_matches('#')));
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Whether the text so far ends with an icon (`{!up} `): its number has its arrow.
fn after_icon(s: &str) -> bool {
    let t = s.trim_end();
    t.ends_with('}') && t.rfind("{!").is_some_and(|k| !t[k..].contains(' '))
}

/// `{$name|text}` colour markup → `{#rrggbb|text}` with the theme's colour.
fn markup_tokens(s: &str, theme: &crate::model::Theme) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("{$") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('|') {
            // `{$good|…}`, or with a size mark after the name: `{$good-|…}`, `{$ink++|…}`
            Some(j) if {
                let name = after[1..j].trim_end_matches(['+', '-']);
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            } => {
                let name_end = 1 + after[1..j].trim_end_matches(['+', '-']).len();
                let c = theme.color(&after[..name_end]);
                out.push_str("{#");
                out.push_str(c.trim_start_matches('#'));
                rest = &after[name_end..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The designer's variables that SQL can use as `:name`: text ones with their
/// placeholders filled in, SQL and data ones as computed (full precision).
/// Names must be plain identifiers. Returns (name, value, kind).
pub fn sql_vars(doc: &Doc) -> Vec<(String, String, String)> {
    doc.variables
        .iter()
        .filter(|v| {
            let n = v.name.trim();
            !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .map(|v| {
            let val = match v.kind.as_str() {
                "sql" | "data" => LOOKUPS.with(|l| l.borrow().get(&format!("varraw:{}", v.name.trim())).cloned()).unwrap_or_default(),
                _ => expand(&v.value, doc),
            };
            (v.name.trim().to_string(), val, v.kind.clone())
        })
        .collect()
}

/// The designer's variables in a text: text ones become their value (which may
/// hold other placeholders), SQL and data ones their resolved value.
fn custom_vars(s: &str, doc: &Doc) -> String {
    let mut out = s.to_string();
    for _ in 0..3 {
        if !out.contains("{{") {
            break;
        }
        let before = out.clone();
        for v in doc.variables.iter().filter(|v| !v.name.trim().is_empty()) {
            let tok = format!("{{{{{}}}}}", v.name.trim());
            if !out.contains(&tok) {
                continue;
            }
            let val = match v.kind.as_str() {
                "sql" | "data" => LOOKUPS.with(|l| l.borrow().get(&format!("var:{}", v.name.trim())).cloned()).unwrap_or_else(|| "…".into()),
                _ => v.value.clone(),
            };
            out = out.replace(&tok, &val);
        }
        if out == before {
            break;
        }
    }
    out
}

pub fn expand(s: &str, doc: &Doc) -> String {
    let out = expand_vars(s, doc);
    // colour names that came in with the data (`{${{row.tone}}|…}`) resolve once it is in
    if out.contains("{$") { markup_tokens(&out, &doc.theme) } else { out }
}

fn expand_vars(s: &str, doc: &Doc) -> String {
    let rowed;
    let s = if s.contains("{{row.") {
        rowed = ROW.with(|r| fill_row(s, &r.borrow(), false));
        rowed.as_str()
    } else {
        s
    };
    let custom;
    let s = if !doc.variables.is_empty() && s.contains("{{") {
        custom = custom_vars(s, doc);
        custom.as_str()
    } else {
        s
    };
    let owned;
    let s = if s.contains("{$") {
        owned = markup_tokens(s, &doc.theme);
        owned.as_str()
    } else {
        s
    };
    if !s.contains("{{") {
        return s.to_string();
    }
    let p = &doc.params;
    let r = period::period(p);
    // the number of pages shown (pages can hide for some data); not inside a page's own condition
    let owned_pages;
    let s = if s.contains("{{pages}}") && doc.pages.iter().all(|p| !p.show_if.contains("{{pages}}")) {
        owned_pages = s.replace("{{pages}}", &page_numbers(doc).iter().filter(|n| n.is_some()).count().to_string());
        owned_pages.as_str()
    } else {
        s
    };
    let mut out = s
        .replace("{{period}}", &period::label(p))
        .replace("{{compare}}", &period::compare_label(p).unwrap_or_default())
        .replace("{{start}}", &period::fmt_date(r.start))
        .replace("{{end}}", &period::fmt_date(r.end))
        .replace("{{title}}", &doc.meta.title)
        .replace("{{subtitle}}", &doc.meta.subtitle)
        .replace("{{organization}}", &doc.meta.organization)
        .replace("{{author}}", &doc.meta.author)
        .replace("{{today}}", &period::fmt_date(chrono::Local::now().date_naive()));
    for d in &doc.param_defs {
        let v = p.values.get(&d.name).filter(|v| !v.is_empty()).map(|v| crate::model::param_values(v).join(", ")).filter(|v| !v.is_empty()).unwrap_or_else(|| "All".into());
        out = out.replace(&format!("{{{{{}}}}}", d.name), &v);
    }
    if out.contains("{{data:") || out.contains("{{sql:") {
        LOOKUPS.with(|l| {
            let l = l.borrow();
            for (k, v) in l.iter().filter(|(k, _)| !k.starts_with("sql:")) {
                out = out.replace(&format!("{{{{data:{k}}}}}"), v);
            }
            // queries: replace each {{sql: …}} by its resolved value
            let mut from = 0;
            while let Some(i) = out[from..].find("{{sql:").map(|i| i + from) {
                let Some(j) = out[i..].find("}}") else { break };
                let q = out[i + 6..i + j].trim().to_string();
                let v = l.get(&format!("sql:{q}")).cloned().unwrap_or_else(|| "…".into());
                out.replace_range(i..i + j + 2, &v);
                from = i + v.len();
            }
        });
    }
    out
}

/// One line of a table of contents.
#[derive(Serialize, Clone)]
pub struct TocEntry {
    /// "section" or "page"
    pub kind: &'static str,
    pub text: String,
    pub page: usize,
    /// The page's place in the document (0-based), when hidden pages make it differ from its number.
    #[serde(default)]
    pub index: usize,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Text as it reads on the page: placeholders filled, markup removed, one line.
pub fn plain(s: &str, doc: &Doc) -> String {
    let t = expand(s, doc);
    let t = match text::parse_rich(&t) {
        Some(runs) => runs.iter().map(|r| r.text.as_str()).collect::<String>(),
        None => t,
    };
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A page's main heading as (title, subtitle): the largest heading with words
/// in it (not a figure), the topmost if several are as large.
pub fn page_heading(doc: &Doc, p: &Page) -> Option<(String, String)> {
    let mut best: Option<(f64, u32, String, String)> = None;
    for w in p.widgets.iter().filter(|w| w.kind == "heading") {
        let e = crate::model::effective(&doc.theme, w);
        let text = plain(e.opt_str("text").unwrap_or(""), doc);
        if !text.chars().any(|c| c.is_alphabetic()) {
            continue;
        }
        let size = font(doc, Some(&e), "heading").size;
        let better = best.as_ref().is_none_or(|(s, y, _, _)| size > *s + 0.1 || ((size - *s).abs() <= 0.1 && w.y < *y));
        if better {
            best = Some((size, w.y, text, plain(e.opt_str("subtitle").unwrap_or(""), doc)));
        }
    }
    best.map(|(_, _, t, s)| (t, s))
}

/// Pages a table of contents can list: not continuation pages, not the contents itself.
fn listable(p: &Page) -> bool {
    !p.auto && !p.widgets.iter().any(|w| w.kind == "toc")
}

/// The entries of a contents widget. `opts.toc` decides where titles and
/// descriptions come from ("pages": the page settings, "headings": each page's
/// main heading, so the contents follows the pages as they are edited) and
/// whether pages without a section are listed too.
pub fn toc(doc: &Doc, opts: Option<&Json>) -> Vec<TocEntry> {
    // the theme studio and the widget studio show sample sections
    if doc.kind == DocKind::Theme || !doc.widget_studio.is_empty() {
        return crate::theme_studio::sample_toc(&doc.theme.locale);
    }
    let o = opts.and_then(|o| o.get("toc"));
    let get = |k: &str| o.and_then(|o| o.get(k)).and_then(|v| v.as_str()).unwrap_or("");
    let from_headings = get("titles") == "headings";
    let desc_headings = get("descFrom") == "headings";
    let all = get("include") == "all";
    let mut out = Vec::new();
    let mut last: Option<String> = None;
    let numbers = page_numbers(doc);
    for (i, p) in doc.pages.iter().enumerate() {
        let Some(num) = numbers[i] else { continue };
        if !listable(p) || p.toc_hidden || p.plain || (p.section.is_empty() && !all) {
            continue;
        }
        if !p.section.is_empty() && last.as_deref() != Some(p.section.as_str()) {
            out.push(TocEntry { kind: "section", text: p.section.clone(), page: num, index: i, description: String::new() });
        }
        last = Some(p.section.clone());
        let (text, description) = with_row(&p.row, || {
            let head = if from_headings || desc_headings { page_heading(doc, p) } else { None };
            let text = match &head {
                Some((t, _)) if from_headings => t.clone(),
                _ => expand(&p.title, doc),
            };
            let description = match &head {
                Some((_, s)) if desc_headings && !s.is_empty() => s.clone(),
                _ => expand(&p.description, doc),
            };
            (text, description)
        });
        out.push(TocEntry { kind: "page", text, page: num, index: i, description });
    }
    out
}

/// Grid rows needed to give `h` points of height.
pub fn rows_for(doc: &Doc, h: f64) -> u32 {
    let (_, ch) = doc.page.cell();
    (((h + doc.page.gap) / (ch + doc.page.gap)) - 1e-6).ceil().max(1.0) as u32
}

/// Title icons against their set size (the renderer draws them by the same factor).
const TITLE_ICON: f64 = 0.85;


/// The default inset of every widget: S. (M is 10, L 16; None is 0, for any widget.)
pub const PAD_S: f64 = 6.0;

/// The inset inside a widget: its own (None included), else S — or, in a card, a theme's roomier card inset
/// (Bold Blocks' colour blocks). A short card (a one-line total) takes a thinner default.
fn frame_pad(doc: &Doc, w: &Widget, frame: &Frame, card: bool) -> f64 {
    if let Some(p) = w.style_f64("pad") {
        // on a card at least S: a view made without a card (pad 0) keeps its title and icon off the card's edge
        // once the card is turned on
        return if card { p.clamp(PAD_S, 30.0) } else { p.clamp(0.0, 30.0) };
    }
    // a cover or another plain page has no cards to line up with: a bare widget there keeps no inset
    if !card && doc.find_widget(&w.id).is_some_and(|(pi, _)| doc.pages[pi].plain) {
        return 0.0;
    }
    let def = if card && doc.theme.card_pad > PAD { doc.theme.card_pad } else { PAD_S };
    // (a fixed rule above a short height: tables sized from their content stay stable)
    if card && frame.h < 48.0 { def.min((frame.h * 0.16).max(3.0)) } else { def }
}

fn card_on(w: &Widget, theme_card: &str) -> bool {
    match w.style.get("card").and_then(|v| v.as_bool()) {
        Some(b) => b,
        None => match w.kind.as_str() {
            "heading" | "divider" | "image" | "text" => false,
            _ => theme_card != "plain" || w.kind == "comment",
        },
    }
}

struct Header {
    title: Option<String>,
    sub: Option<String>,
    title_y: f64,
    sub_y: f64,
    height: f64,
}

fn header(doc: &Doc, w: &Widget, frame: &Frame, card: bool, tf: &Font, sf: &Font) -> Header {
    // a title can be hidden and kept (style.hideTitle)
    let show_title = !w.title.is_empty() && !matches!(w.kind.as_str(), "heading" | "divider") && w.style.get("hideTitle").and_then(|v| v.as_bool()) != Some(true);
    let pad = if card || show_title { frame_pad(doc, w, frame, card) } else { 0.0 };
    let inner = frame.w - 2.0 * pad;
    // titles wrap onto a second line before they're cut short
    let title_lines: Vec<String> = if show_title {
        let full = tf.apply(&expand(&w.title, doc)).into_owned();
        let mut lines = text::wrap(&full, inner, tf);
        if lines.len() > 2 {
            let rest = lines[1..].join(" ");
            lines.truncate(1);
            lines.push(text::ellipsize(&rest, inner, tf));
        }
        lines
    } else {
        Vec::new()
    };
    // the subtitle may sit at the right end of the title's line (a result
    // beside its label, e.g. "Position on return · ahead of 88%")
    let beside = show_title && !w.subtitle.is_empty() && w.opts.get("subSide").and_then(|v| v.as_bool()) == Some(true);
    if beside {
        let sub = expand(&w.subtitle, doc);
        let sw = text::width_f(&sub, sf).min(inner * 0.6);
        let sub = text::ellipsize(&sub, sw + 1.0, sf);
        let full = tf.apply(&expand(&w.title, doc)).into_owned();
        let title = text::ellipsize(&full, (inner - sw - 12.0).max(20.0), tf);
        let line = tf.size.max(sf.size);
        let title_y = frame.y + pad + line * 0.96;
        return Header { title: Some(title), sub: Some(sub), title_y, sub_y: title_y, height: pad + line * 1.37 + title_gap(w) };
    }
    let title = show_title.then(|| title_lines.join("\n"));
    let sub = (show_title && !w.subtitle.is_empty()).then(|| text::ellipsize(&expand(&w.subtitle, doc), inner, sf));
    // an icon chip before the title is taller than its line: room for it above, even with no padding
    let chip = w.style.get("titleIcon").filter(|t| t.get("off").and_then(|v| v.as_bool()) != Some(true) && t.get("icon").is_some())
        .map(|t| tf.size * t.get("size").and_then(|v| v.as_f64()).unwrap_or(2.1) * TITLE_ICON).unwrap_or(0.0);
    let lift = if show_title { (chip / 2.0 - tf.size * 0.6 - pad).max(0.0) } else { 0.0 };
    let mut y = frame.y + pad + lift;
    let title_y = y + tf.size * 0.96;
    if title.is_some() {
        y += tf.size * 1.37 + (title_lines.len().saturating_sub(1)) as f64 * tf.size * 1.2;
    }
    let sub_y = y + sf.size * 0.96;
    if sub.is_some() {
        y += sf.size * 1.375;
    }
    if title.is_some() {
        y += title_gap(w);
    }
    Header { title, sub, title_y, sub_y, height: y - frame.y }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn widget_vm(doc: &Doc, w: &Widget, frame: Frame, data: Option<Json>) -> WidgetVm {
    widget_vm_chain(doc, w, frame, data, Chain { offset: 0, last: true, part: 0 })
}

pub fn widget_vm_chain(doc: &Doc, w: &Widget, frame: Frame, data: Option<Json>, chain: Chain) -> WidgetVm {
    let eff = crate::model::effective(&doc.theme, w);
    let has_variant = matches!(eff, std::borrow::Cow::Owned(_));
    let w: &Widget = &eff;
    let fonts = fonts_for(doc, Some(w));
    let card = card_on(w, &doc.theme.card);
    let hd = header(doc, w, &frame, card, &fonts["title"], &fonts["subtitle"]);
    let (title, subtitle, title_y, sub_y, head_h) = (hd.title, hd.sub, hd.title_y, hd.sub_y, hd.height);
    // every widget has the default inset (S) unless it sets its own
    let framed = card || title.is_some();
    let pad = frame_pad(doc, w, &frame, card);
    // a bare widget's default inset is at its sides only (its content lines up with the others' and keeps its
    // rows); one it sets itself goes all round
    let vpad = if framed || w.style_f64("pad").is_some() { pad } else { 0.0 };
    let top = if framed { 0.0 } else { vpad };
    let comment_bar = if w.kind == "comment" { 4.0 } else { 0.0 };
    let body = Frame {
        x: frame.x + pad + comment_bar,
        y: frame.y + head_h + top,
        w: (frame.w - 2.0 * pad - comment_bar).max(1.0),
        h: (frame.h - head_h - vpad - top).max(1.0),
    };
    let mut texts = Vec::new();
    let mut table = None;
    let mut need_h = None;
    let mut toc_vm = None;
    let mut deco = Vec::new();
    match w.kind.as_str() {
        "toc" => {
            let entries = toc(doc, Some(&w.opts));
            let lh = fonts["body"].size * 2.0;
            need_h = Some(rows_for(doc, head_h + entries.len() as f64 * lh + pad));
            toc_vm = Some(entries);
        }
        "heading" => {
            // a heading in a card is inset like any card content
            let p = frame_pad(doc, w, &frame, card);
            // a short card (a one-line total) is inset at the sides only and centres its line
            let short = card && frame.h < 48.0;
            let vp = if short { 0.0 } else { p };
            let inner = if card { Frame { x: frame.x + p, y: frame.y + vp, w: (frame.w - 2.0 * p).max(1.0), h: (frame.h - 2.0 * vp).max(1.0) } } else { frame };
            let n0 = texts.len();
            let (h, mut d) = heading_vm(doc, w, &inner, &fonts, &mut texts);
            if short && h < inner.h {
                let dy = (inner.h - h) / 2.0;
                for t in texts.iter_mut().skip(n0) {
                    t.y += dy;
                }
                for x in d.iter_mut() {
                    x.y += dy;
                }
            }
            let h = h + if card { 2.0 * vp } else { 0.0 };
            deco = d;
            need_h = Some(rows_for(doc, h));
        }
        "text" | "comment" => {
            let bf = &fonts["body"];
            let mut y = body.y;
            let mut h = head_h;
            let signed = w.kind == "comment" && w.opts.get("signed").and_then(|v| v.as_bool()) == Some(true);
            if signed {
                // a note from the banker: initials, who it is from, the date
                let author = w.opt_str("author").unwrap_or("").trim().to_string();
                let cf = &fonts["caption"];
                let ef = &fonts["eyebrow"];
                let d = (bf.size * 2.9).max(16.0);
                let mut nf = bf.clone();
                nf.weight = 700;
                let mut inf = nf.clone();
                inf.size = d * 0.38;
                inf.color = "#ffffff".into();
                inf.tracking = 0.02;
                // the note's colour: its own, else the theme's dark ink (the accent can read as a warning)
                let mark = w.style.get("color").and_then(|v| v.as_str()).map(|c| doc.theme.color(c))
                    .unwrap_or_else(|| doc.theme.token("ink2").unwrap_or_else(|| doc.theme.ink.clone()));
                deco.push(Deco::rect(body.x, y, d, d, &mark, d / 2.0));
                deco.push(Deco::text(body.x + d / 2.0, y + d / 2.0 + inf.size * 0.36, initials(&author), &inf, "middle"));
                let tx = body.x + d + bf.size * 0.8;
                let label = match doc.theme.locale.get(..2) {
                    Some("ru") => "Комментарий вашего банкира",
                    Some("de") => "Notiz Ihres Bankers",
                    _ => "A note from your banker",
                };
                deco.push(Deco::text(tx, y + d * 0.42, ef.apply(label).into_owned(), ef, ""));
                if !author.is_empty() {
                    deco.push(Deco::text(tx, y + d * 0.42 + nf.size * 1.25, author.clone(), &nf, ""));
                }
                if let Some(date) = w.opt_str("date").filter(|s| !s.is_empty()) {
                    deco.push(Deco::text(body.x + body.w, y + d * 0.42, date.to_string(), cf, "end"));
                }
                y += d + bf.size * 0.9;
                h += d + bf.size * 0.9;
            } else if w.kind == "comment" && title.is_none() {
                // byline instead of a title
                let by = [w.opt_str("author").unwrap_or(""), w.opt_str("date").unwrap_or("")]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let by = if by.is_empty() { "Comment".to_string() } else { format!("Comment · {by}") };
                let cf = &fonts["caption"];
                let bb = text::block(&by, body.w, cf);
                let bh = bb.height + 5.0;
                texts.push(TextVm { x: body.x, y, block: bb, color: Some(cf.color.clone()) });
                y += bh;
                h += bh;
            }
            let mut txt = expand(w.opt_str("text").unwrap_or(""), doc);
            if w.opts.get("arrows").and_then(|v| v.as_bool()) == Some(true) {
                txt = auto_arrows(&txt, &doc.theme.color(&doc.theme.good), &doc.theme.color(&doc.theme.bad));
            }
            let mut b = text::block(&txt, body.w, bf);
            align_block(&mut b, body.w, bf, w.opt_str("align"));
            h += b.height + pad;
            texts.push(TextVm { x: body.x, y, block: b, color: Some(bf.color.clone()) });
            need_h = Some(rows_for(doc, h));
        }
        "kpi" => {
            // what the figure and the lines under it take (as the drawing lays them out, the figure at
            // its smallest — 75% of its size): more than the card has, and it asks for more rows
            let e = crate::model::effective(&doc.theme, w);
            let v = &e.opts["viz"];
            let on = |k: &str| v.get(k).and_then(|x| x.as_bool());
            if on("inline") != Some(true) {
                let kf = &fonts["kpi"];
                let cf = &fonts["caption"];
                let mut size = v.get("valueSize").and_then(|x| x.as_f64()).filter(|x| *x > 0.0)
                    .or(Some(kf.size).filter(|x| *x > 0.0)).unwrap_or_else(|| (body.h * 0.44).clamp(13.0, 30.0));
                // the figure first gives way to the width, as the drawing does (a long amount in a narrow card)
                if let Some(val) = data.as_ref().and_then(|d| d["kpi"]["value"].as_f64()) {
                    let fmt = data.as_ref().and_then(|d| d["columns"].as_array().and_then(|c| c.iter().find(|c| c["role"] == "measure")).and_then(|c| c["format"].as_str()).map(String::from)).unwrap_or_default();
                    let text = match fmt.as_str() {
                        "percent" => format!("+{}%", crate::data::fmt_num_locale((val * 1000.0).round() / 10.0)),
                        "currency" => format!("+{} ₽", crate::data::fmt_num_locale(val.round())),
                        _ => format!("+{}", crate::data::fmt_num_locale(val.round())),
                    };
                    let wide = text::width_f(&text, &Font { size, ..kf.clone() });
                    if wide > body.w && wide > 0.0 {
                        size = (size * body.w / wide).max(8.0);
                    }
                }
                let floor = (size * 0.75).max(8.0);
                let n_vals = data.as_ref().and_then(|d| d["kpi"]["values"].as_array().map(|a| a.len())).unwrap_or(1);
                let has_delta = data.as_ref().is_some_and(|d| !d["kpi"]["pct"].is_null() || !d["kpi"]["delta"].is_null());
                let need = if on("breakdown") == Some(true) && n_vals > 1 {
                    floor * 0.92 + cf.size + 7.5 + (n_vals - 1) as f64 * (cf.size * 2.1).max(12.0)
                } else {
                    floor * 1.1 + if v.get("note").and_then(|x| x.as_str()).is_some_and(|n| !n.is_empty()) { cf.size + 6.0 } else { 0.0 }
                        + if on("delta") != Some(false) && has_delta { cf.size + 6.0 } else { 0.0 }
                };
                // the estimate runs a little over what is drawn: it is cut off only when it would cross the card's
                // edge (into the padding under the body is still inside the card)
                if need > body.h + (frame.y + frame.h - body.y - body.h).clamp(0.0, 6.0) + 0.5 {
                    need_h = Some(rows_for(doc, (body.y - frame.y) + need + pad));
                }
            }
        }
        "table" => {
            let (mut tv, need) = table_vm(w, &fonts, &frame, &body, data.as_ref(), chain);
            // a piece held to fewer rows: the rest go on with the widgets linked to its end
            if let Some(&cap) = doc.keep.get(&w.id) {
                if cap > 0 && tv.visible > cap {
                    tv.heights.truncate(cap as usize);
                    for v in tv.wrapped.values_mut() {
                        v.truncate(cap as usize);
                    }
                    tv.visible = cap;
                    tv.continued = !chain.last && tv.offset + cap < tv.rows;
                    tv.totals = tv.totals && tv.offset + cap >= tv.rows;
                }
            }
            need_h = Some(rows_for(doc, need));
            table = Some(tv);
        }
        _ => {}
    }
    WidgetVm { id: w.id.clone(), frame, body, card, title, subtitle, title_y, sub_y, data, texts, table, need_h, fonts, toc: toc_vm, deco, viz_text: viz_texts(doc, w),
        eff: has_variant.then(|| json!({"opts": w.opts, "style": w.style})), moved: false, parts: Vec::new() }
}

/// Right- or centre-aligns the lines of a block within `width`.
fn align_block(b: &mut Block, width: f64, f: &Font, align: Option<&str>) {
    let k = match align {
        Some("right") => 1.0,
        Some("center") => 0.5,
        _ => return,
    };
    for l in &mut b.lines {
        let lw = text::line_width(l, f);
        l.x += (width - l.x - lw).max(0.0) * k;
    }
}

fn viz_texts(doc: &Doc, w: &Widget) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(o) = w.opts.get("viz").and_then(|v| v.as_object()) {
        for (k, v) in o {
            if let Some(t) = v.as_str().filter(|t| t.contains("{{")) {
                out.insert(k.clone(), expand(t, doc));
            }
        }
    }
    out
}

/// A heading: an optional eyebrow line (with a dark badge chip), the title
/// (with *italic* emphasis), a subtitle, navigation between the pages of its
/// group, a note on the right and a rule underneath. Returns the height used.
fn heading_vm(doc: &Doc, w: &Widget, frame: &Frame, fonts: &BTreeMap<&'static str, Font>, texts: &mut Vec<TextVm>) -> (f64, Vec<Deco>) {
    let th = &doc.theme;
    let hf = &fonts["heading"];
    let ef = &fonts["eyebrow"];
    let mut deco = Vec::new();
    let mut y = frame.y;
    // eyebrow line: [badge] EYEBROW
    let badge = expand(w.opt_str("badge").unwrap_or(""), doc);
    let eyebrow = expand(w.opt_str("eyebrow").unwrap_or(""), doc);
    if !badge.is_empty() || !eyebrow.is_empty() {
        let mut x = frame.x;
        let mut line_h = ef.size * 1.3;
        if !badge.is_empty() {
            let mut bf = ef.clone();
            bf.size = w.opt_f64("badgeSize").unwrap_or((ef.size * 1.5).max(9.0));
            bf.weight = 800;
            bf.tracking = 0.02;
            bf.upper = false;
            // solid (a filled chip), outline (a frame in the badge colour) or text (no chip)
            let style = w.opt_str("badgeStyle").unwrap_or("solid");
            let fill = th.color(w.opt_str("badgeFill").unwrap_or(&th.ink));
            bf.color = match style {
                "solid" => th.color(w.opt_str("badgeColor").unwrap_or(&th.page_bg)),
                _ => th.color(w.opt_str("badgeColor").unwrap_or(&fill)),
            };
            if let Some(wt) = w.opt_f64("badgeWeight") {
                bf.weight = wt as u16;
            }
            let bh = bf.size * 1.9;
            let pad = if style == "text" { 0.0 } else { bf.size * 1.3 };
            let bw = text::width_f(&badge, &bf) + pad;
            let r = w.opt_f64("badgeRadius").unwrap_or(bf.size * 0.45).clamp(0.0, bh / 2.0);
            match style {
                "text" => {}
                "outline" => {
                    let sw = 0.9;
                    deco.push(Deco::rect(x, y, bw, bh, &fill, r));
                    deco.push(Deco::rect(x + sw, y + sw, bw - 2.0 * sw, bh - 2.0 * sw, &th.page_bg, (r - sw).max(0.0)));
                }
                _ => deco.push(Deco::rect(x, y, bw, bh, &fill, r)),
            }
            deco.push(Deco::text(x + bw / 2.0, y + bh / 2.0 + bf.size * 0.36, badge.clone(), &bf, "middle"));
            x += bw + ef.size * 1.2;
            line_h = line_h.max(bh);
        }
        if !eyebrow.is_empty() {
            let t = ef.apply(&eyebrow).into_owned();
            let t = text::ellipsize(&t, frame.x + frame.w - x, ef);
            let (ex, anchor) = match w.opt_str("align") {
                Some("right") => (frame.x + frame.w, "end"),
                Some("center") => (frame.x + frame.w / 2.0, "middle"),
                _ => (x, "start"),
            };
            deco.push(Deco::text(ex, y + line_h / 2.0 + ef.size * 0.36, t, ef, anchor));
        }
        y += line_h + ef.size * 0.8;
    }
    // the note on the right takes room from the title
    let meta = expand(w.opt_str("meta").unwrap_or(""), doc);
    let mf = &fonts["caption"];
    let meta_w = if meta.is_empty() { 0.0 } else { (frame.w * 0.62).min(meta.split('\n').map(|l| text::width_f(l, mf)).fold(0.0, f64::max) * 1.03 + 4.0) };
    let title_w = frame.w - if meta_w > 0.0 { meta_w + 16.0 } else { 0.0 };
    let align = w.opt_str("align");
    let txt = expand(w.opt_str("text").unwrap_or(&w.title), doc);
    let mut b = text::block_lh(&txt, title_w, hf, 1.15);
    align_block(&mut b, title_w, hf, align);
    let title_top = y;
    let last_base = b.lines.last().map(|l| title_top + l.y).unwrap_or(title_top + hf.size);
    y += b.height;
    texts.push(TextVm { x: frame.x, y: title_top, block: b, color: Some(hf.color.clone()) });
    if meta_w > 0.0 {
        // a styled block (bold, colours, sizes in the note), right-aligned, its last line on the title's baseline
        let mut mb = text::block_lh(&meta, meta_w, mf, 1.35);
        align_block(&mut mb, meta_w, mf, Some("right"));
        let lines = mb.lines.len().max(1) as f64;
        let top = last_base - mf.size * 0.96 - (lines - 1.0) * mb.line_h;
        texts.push(TextVm { x: frame.x + frame.w - meta_w, y: top, block: mb, color: Some(mf.color.clone()) });
    }
    let sub_txt = expand(w.opt_str("subtitle").unwrap_or(""), doc);
    if !sub_txt.is_empty() {
        let sf = &fonts["heading_sub"];
        let mut sb = text::block(&sub_txt, title_w, sf);
        align_block(&mut sb, title_w, sf, align);
        y += 4.0;
        let h = sb.height;
        texts.push(TextVm { x: frame.x, y, block: sb, color: Some(sf.color.clone()) });
        y += h;
    }
    // page navigation: OVERVIEW · HOLDINGS 1 · HOLDINGS 2
    if w.opt_bool("nav").unwrap_or(false) {
        let items = nav_pages(doc, w);
        if items.len() > 1 {
            let mut nf = ef.clone();
            nf.size = ef.size * 0.95;
            nf.color = th.muted.clone();
            nf.tracking = 0.08;
            let mut cur = nf.clone();
            cur.color = th.ink.clone();
            cur.weight = nf.weight.max(700);
            y += nf.size * 0.9;
            let base = y + nf.size * 0.96;
            let mut x = frame.x;
            let gap = nf.size * 1.6;
            for (i, (label, on)) in items.iter().enumerate() {
                if i > 0 {
                    deco.push(Deco::text(x + gap / 2.0, base, "·".to_string(), &nf, "middle"));
                    x += gap;
                }
                let f = if *on { &cur } else { &nf };
                let t = f.apply(label).into_owned();
                let tw = text::width_f(&t, f);
                deco.push(Deco::text(x, base, t, f, "start"));
                x += tw;
                let _ = i;
            }
            y += nf.size * 1.3;
        }
    }
    // a rule under the heading, at the bottom of its frame
    let rule = w.opt_f64("rule").unwrap_or(0.0);
    if rule > 0.0 {
        let color = w.opt_str("ruleColor").unwrap_or(&th.ink).to_string();
        y += 8.0 + rule;
        let ry = (frame.y + frame.h - rule).max(y - rule);
        deco.push(Deco::line(frame.x, ry, frame.w, rule, &color));
    }
    (y - frame.y, deco)
}


/// Layout items for one page, with content needs measured.
pub fn items(doc: &Doc, page: &Page, results: &HashMap<String, Json>) -> Vec<Item> {
    items_with(doc, page, results, &chains(doc, results))
}

fn items_with(doc: &Doc, page: &Page, results: &HashMap<String, Json>, ch: &HashMap<String, Chain>) -> Vec<Item> {
    let designer = doc.kind == DocKind::Template;
    page.widgets
        .iter()
        // widgets linked to a table's end that go after its last rows on a later page aren't here, nor
        // dividers drawn in a gap (they take no room)
        .filter(|w| !doc.moved.contains(&w.id) && !crate::model::in_gap(&doc.theme, w))
        .map(|w| {
            let frame = doc.page.frame(w.x, w.y, w.w, eff_h(doc, w));
            let chain = ch.get(&w.id).copied().unwrap_or(Chain { offset: 0, last: true, part: 0 });
            let data = results.get(continues(w).unwrap_or(&w.id)).or_else(|| results.get(&w.id)).cloned();
            let vm = widget_vm_chain(doc, w, frame, data, chain);
            let in_chain = ch.contains_key(&w.id);
            // a 100% bar is meant to be a single thin row
            let eff = crate::model::effective(&doc.theme, w);
            let kind = if w.kind == "bar" && eff.opt_str("orientation") == Some("share") { "sharebar".to_string() } else { w.kind.clone() };
            Item {
                id: w.id.clone(),
                kind,
                rect: layout::Rect { x: w.x, y: w.y, w: w.w, h: w.h },
                // the designer moves locked widgets freely; users can't. A widget placed by
                // hand stays where it was put when the page is tidied after other changes.
                locked: (w.locked && !designer) || w.opts.get("placed").and_then(|v| v.as_bool()) == Some(true),
                need_h: vm.need_h,
                // a piece that hands rows on to the next page isn't hiding anything
                table_rows: vm.table.as_ref().map(|t| {
                    if in_chain && !chain.last { (t.visible, t.visible) } else { (t.visible, t.rows - t.offset) }
                }),
            }
        })
        .collect()
}

pub fn grid(doc: &Doc) -> Grid {
    Grid { cols: doc.page.cols.max(1), rows: doc.page.rows.max(1) }
}

pub fn pages(doc: &Doc, results: &HashMap<String, Json>) -> Vec<PageVm> {
    let g = grid(doc);
    let ch = chains(doc, results);
    let all_items: Vec<Vec<Item>> = doc.pages.iter().map(|p| with_row(&p.row, || items_with(doc, p, results, &ch))).collect();
    let titles = |id: &str| -> String {
        doc.find_widget(id)
            .map(|(pi, wi)| {
                let w = &doc.pages[pi].widgets[wi];
                if let Some(n) = w.opt_str("name").map(str::trim).filter(|n| !n.is_empty()) {
                    n.to_string()
                } else if !w.title.is_empty() {
                    w.title.clone()
                } else if let Some(ds) = w.binding.as_ref().and_then(|b| doc.dataset(&b.dataset)) {
                    // an untitled figure goes by what it shows
                    format!("{} · {}", kind_name(&w.kind), ds.name)
                } else {
                    let t = expand(w.opt_str("text").unwrap_or(""), doc);
                    let first = t.lines().next().unwrap_or("").trim().to_string();
                    if first.is_empty() {
                        kind_name(&w.kind).to_string()
                    } else if first.chars().count() > 32 {
                        format!("{}…", first.chars().take(30).collect::<String>())
                    } else {
                        first
                    }
                }
            })
            .unwrap_or_default()
    };
    let numbers = page_numbers(doc);
    doc.pages
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            let next = doc.pages.get(pi + 1).map(|n| (n.id.as_str(), all_items[pi + 1].as_slice()));
            let ctx = PageCtx { page_id: &p.id, next, titles: &titles };
            let issues = layout::analyze(&all_items[pi], g, &ctx);
            with_row(&p.row, || PageVm {
                id: p.id.clone(),
                hidden: numbers[pi].is_none(),
                number: numbers[pi].unwrap_or(0),
                title: expand(&p.title, doc),
                copy: p.copy,
                copies: p.copies,
                copy_label: repeat_label(doc, p),
                heading: if listable(p) { page_heading(doc, p) } else { None },
                widgets: {
                    // a house's colour blocks: the widgets of fillCycle views take them in turn, in reading order
                    let bl = blocks(doc);
                    let mut cyc: Vec<(u32, u32, usize)> = Vec::new();
                    if !bl.is_empty() {
                        for (i, w) in p.widgets.iter().enumerate() {
                            if crate::model::effective(&doc.theme, w).style.get("fillCycle").and_then(|v| v.as_bool()) == Some(true) {
                                cyc.push((w.y, w.x, i));
                            }
                        }
                        cyc.sort();
                    }
                    p.widgets
                        .iter()
                        .enumerate()
                        .map(|(i, w)| {
                            let chain = ch.get(&w.id).copied().unwrap_or(Chain { offset: 0, last: true, part: 0 });
                            let data = results.get(continues(w).unwrap_or(&w.id)).or_else(|| results.get(&w.id)).cloned();
                            let frame = doc.page.frame(w.x, w.y, w.w, eff_h(doc, w));
                            let mut vm = match cyc.iter().position(|c| c.2 == i) {
                                Some(k) => {
                                    let b = on_block(doc, w, &bl[k % bl.len()]);
                                    let mut vm = widget_vm_chain(doc, &b, frame, data, chain);
                                    // the drawing reads the widget's style from here
                                    let e = crate::model::effective(&doc.theme, &b);
                                    vm.eff = Some(serde_json::json!({"opts": e.opts, "style": e.style}));
                                    vm
                                }
                                None => {
                                    // an accent bar on the left of a figure set at the right edge would stand apart from it
                                    let e = crate::model::effective(&doc.theme, w);
                                    let right = e.opts.pointer("/viz/kpiAlign").and_then(|v| v.as_str()) == Some("right") || e.opts.get("align").and_then(|v| v.as_str()) == Some("right");
                                    if right && e.style.get("rule").is_some() {
                                        let mut c = w.clone();
                                        if !c.style.is_object() { c.style = serde_json::json!({}); }
                                        c.style["rule"] = Json::Null;
                                        let mut vm = widget_vm_chain(doc, &c, frame, data, chain);
                                        let e2 = crate::model::effective(&doc.theme, &c);
                                        let mut st = e2.style.clone();
                                        if let Some(o) = st.as_object_mut() { o.remove("rule"); }
                                        vm.eff = Some(serde_json::json!({"opts": e2.opts, "style": st}));
                                        vm
                                    } else {
                                        widget_vm_chain(doc, w, frame, data, chain)
                                    }
                                }
                            };
                            vm.moved = doc.moved.contains(&w.id);
                            if !w.parts.is_empty() {
                                vm.parts = part_vms(doc, w, &vm, results);
                            }
                            vm
                        })
                        .collect()
                },
                issues,
                // the cover (first page) and the closing page (last) on the theme's papers, when it has them
                fill: {
                    let tok = |k: &str| doc.theme.token(k);
                    if pi == 0 { tok("coverFill") } else if pi + 1 == doc.pages.len() { tok("backFill") } else { None }
                },
            })
        })
        .collect()
}

/// What a repeated page's copy is for: its row's label column (or the first value).
pub fn repeat_label(doc: &Doc, p: &Page) -> String {
    if p.row.is_empty() {
        return String::new();
    }
    let src = if p.repeat_of.is_empty() { Some(p) } else { doc.pages.iter().find(|q| q.id == p.repeat_of) };
    let col = src.map(|q| q.repeat.label.as_str()).unwrap_or("");
    p.row.get(col).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| p.row.values().next().cloned().unwrap_or_default())
}

pub fn kind_name(kind: &str) -> &'static str {
    match kind {
        "kpi" => "KPI card",
        "bar" => "Bar chart",
        "line" => "Line chart",
        "area" => "Area chart",
        "pie" => "Donut chart",
        "table" => "Table",
        "heading" => "Heading",
        "comment" => "Comment",
        "text" => "Text",
        "divider" => "Divider",
        "image" => "Image",
        "treemap" => "Treemap",
        "waterfall" => "Waterfall",
        "tiles" => "Tiles",
        "scale" => "Dot scale",
        "toc" => "Contents",
        "composite" => "Composite",
        _ => "Widget",
    }
}

pub fn specs() -> Json {
    let kinds = ["kpi", "bar", "line", "area", "pie", "table", "heading", "text", "comment", "divider", "image", "treemap", "waterfall", "tiles", "scale", "toc", "composite"];
    Json::Object(kinds.iter().map(|k| (k.to_string(), json!(layout::spec(k)))).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_piece_repeats_the_subheaders_it_starts_inside() {
        // group A (level 0) · rows · group B · subgroup B1 (level 1) · rows
        let meta = [(true, 0), (false, 1), (false, 1), (true, 0), (true, 1), (false, 2), (false, 2)];
        assert!(carried_groups(&meta, 0).is_empty());
        assert_eq!(carried_groups(&meta, 2), vec![0]);
        assert!(carried_groups(&meta, 3).is_empty(), "starting on a subheader shows it anyway");
        assert_eq!(carried_groups(&meta, 4), vec![3]);
        assert_eq!(carried_groups(&meta, 6), vec![3, 4]);
    }

    use super::*;
    use crate::model::FontSpec;
    use serde_json::json;

    #[test]
    fn fonts_inherit_theme_then_widget() {
        let mut doc = Doc::default();
        doc.theme.font_family = "Georgia".into();
        doc.theme.fonts.insert("title".into(), FontSpec { size: Some(12.0), italic: Some(true), ..Default::default() });
        let w = Widget { kind: "bar".into(), style: json!({"fonts": {"title": {"family": "Courier New", "color": "#ff0000"}}}), ..Default::default() };
        let t = font(&doc, Some(&w), "title");
        assert_eq!((t.family.as_str(), t.size, t.italic, t.color.as_str(), t.weight), ("Courier New", 12.0, true, "#ff0000", 600));
        let body = font(&doc, Some(&w), "body");
        assert_eq!(body.family, "Georgia");
        doc.theme.font_scale = 2.0;
        assert_eq!(font(&doc, None, "footer").size, 14.0);
    }

    #[test]
    fn chart_parts_start_as_the_chart_text_and_take_their_own() {
        let mut doc = Doc::default();
        doc.theme.fonts.insert("chart".into(), FontSpec { size: Some(8.0), family: Some("Georgia".into()), ..Default::default() });
        let w = Widget { kind: "bar".into(), title: "Chart".into(), style: json!({"fonts": {"data_label": {"size": 11, "weight": 800, "color": "#ff0000"}}}), ..Default::default() };
        // unset: the chart text, at the size the drawing always gave the part, in its grey
        let axis = font(&doc, Some(&w), "axis");
        assert_eq!((axis.family.as_str(), axis.size, axis.color.as_str()), ("Georgia", 7.5, doc.theme.muted.as_str()));
        assert_eq!(font(&doc, Some(&w), "legend").size, 8.5);
        // set for the widget: its own, scaled with the theme's text size
        let lab = font(&doc, Some(&w), "data_label");
        assert_eq!((lab.size, lab.weight, lab.color.as_str(), lab.family.as_str()), (11.0, 800, "#ff0000", "Georgia"));
        doc.theme.font_scale = 1.2;
        assert!((font(&doc, Some(&w), "data_label").size - 13.2).abs() < 1e-9);
        // the space under a title is the widget's own when set
        let f = doc.page.frame(0, 0, 4, 3);
        let h0 = widget_vm(&doc, &w, f, None).body.y;
        let mut w2 = w.clone();
        w2.style["titleGap"] = json!(20);
        assert!((widget_vm(&doc, &w2, f, None).body.y - h0 - 16.0).abs() < 0.01);
    }

    #[test]
    fn a_kpi_too_short_for_its_figures_asks_for_rows() {
        let doc = Doc::default();
        let data = json!({"kpi": {"value": 30924336.0, "values": [30924336.0, 2577028.0, 12.0]}, "columns": []});
        let mut w = Widget { kind: "kpi".into(), title: "Регулярный доход".into(), w: 3, h: 2, style: json!({"fonts": {"kpi": {"size": 22}}}),
            opts: json!({"viz": {"breakdown": true}}), ..Default::default() };
        let f = doc.page.frame(0, 0, 3, 2);
        let need = widget_vm(&doc, &w, f, Some(data.clone())).need_h.expect("a short card asks for more rows");
        assert!(need > 2, "{need}");
        // tall enough: nothing asked
        w.h = 6;
        assert!(widget_vm(&doc, &w, doc.page.frame(0, 0, 3, 6), Some(data)).need_h.is_none());
    }

    #[test]
    fn bigger_text_needs_more_rows() {
        let mut doc = Doc::default();
        let mut w = Widget { kind: "text".into(), w: 4, h: 2, opts: json!({"text": "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(6)}), ..Default::default() };
        let f = doc.page.frame(0, 0, 4, 2);
        let small = widget_vm(&doc, &w, f, None).need_h.unwrap();
        w.style = json!({"fonts": {"body": {"size": 16}}});
        let big = widget_vm(&doc, &w, f, None).need_h.unwrap();
        assert!(big > small, "{big} > {small}");
        doc.theme.fonts.insert("body".into(), FontSpec { upper: Some(true), ..Default::default() });
        assert!(widget_vm(&doc, &w, f, None).texts[0].block.lines[0].text.starts_with("LOREM"));
    }
}

#[cfg(test)]
mod arrow_tests {
    use super::*;

    #[test]
    fn signed_numbers_get_arrows() {
        let t = auto_arrows("Доход +6,48% за год, расходы −2,1%; итог 5-6 и (+3 500)", "#166f5e", "#a12a2a");
        assert!(t.contains("{#166f5e|{!up} +6,48%}"), "{t}");
        assert!(t.contains("{#a12a2a|{!down} −2,1%}"), "{t}");
        assert!(t.contains("5-6"), "a range is not a sign: {t}");
        assert!(t.contains("({#166f5e|{!up} +3 500})"), "{t}");
        assert!(auto_arrows("Доход **+6,48%**", "#166f5e", "#a12a2a").contains("**{#166f5e|{!up} +6,48%}**"), "bold numbers too");
        // once is enough
        assert_eq!(auto_arrows(&t, "#166f5e", "#a12a2a"), t);
        let runs = crate::text::parse_rich(&t).unwrap();
        assert!(runs.iter().any(|r| r.icon.as_deref() == Some("up") && r.color.as_deref() == Some("#166f5e")));
    }
}

#[cfg(test)]
mod named_colour_tests {
    use super::*;

    #[test]
    fn theme_colours_by_name_follow_the_theme() {
        let mut doc = Doc::default();
        doc.theme.accent = "#c8102e".into();
        doc.theme.palette = vec!["#111111".into(), "#8ecccb".into()];
        doc.theme.tokens.insert("gainTint".into(), "#dcefe6".into());
        assert_eq!(doc.theme.color("$series2"), "#8ecccb");
        assert_eq!(doc.theme.color("$series9"), "#888888");
        // a widget's colours by name resolve in what it draws
        let w = Widget { kind: "bar".into(), opts: json!({"viz": {"gridColor": "$accent", "colors": {"A": "$series2"}}}), style: json!({"background": "$gainTint", "fonts": {"title": {"color": "$accent"}}}), ..Default::default() };
        let e = crate::model::effective(&doc.theme, &w);
        assert_eq!(e.opts["viz"]["gridColor"], "#c8102e");
        assert_eq!(e.opts["viz"]["colors"]["A"], "#8ecccb");
        assert_eq!(e.style["background"], "#dcefe6");
        assert_eq!(font(&doc, Some(&w), "title").color, "#c8102e");
        // and a header band's fill and rule
        let b = crate::model::Band { fill: "$accent".into(), line_color: "$series1".into(), ..Default::default() };
        let v = band(&doc, &b);
        assert_eq!((v["fill"].as_str(), v["line_color"].as_str()), (Some("#c8102e"), Some("#111111")));
        // a new accent: everything named after it follows
        doc.theme.accent = "#2a78d6".into();
        assert_eq!(crate::model::effective(&doc.theme, &w).opts["viz"]["gridColor"], "#2a78d6");
    }
}

/// The space between a composite's parts, in points.
pub const PART_GAP: f64 = 6.0;

/// Where a composite's part sits: its cells of the composite's grid inside the card's body.
pub fn part_frame(parent: &Widget, body: &Frame, p: &Widget) -> Frame {
    let (cols, rows) = crate::model::part_grid(parent);
    let cw = ((body.w - PART_GAP * (cols as f64 - 1.0)) / cols as f64).max(1.0);
    let rh = ((body.h - PART_GAP * (rows as f64 - 1.0)) / rows as f64).max(1.0);
    let (x, y) = (p.x.min(cols - 1), p.y.min(rows - 1));
    let (w, h) = (p.w.clamp(1, cols - x), p.h.clamp(1, rows - y));
    Frame {
        x: body.x + x as f64 * (cw + PART_GAP),
        y: body.y + y as f64 * (rh + PART_GAP),
        w: w as f64 * cw + (w as f64 - 1.0) * PART_GAP,
        h: h as f64 * rh + (h as f64 - 1.0) * PART_GAP,
    }
}

/// A composite's parts as widgets, each in its frame, with its data.
pub fn part_vms(doc: &Doc, parent: &Widget, vm: &WidgetVm, results: &HashMap<String, Json>) -> Vec<WidgetVm> {
    parent
        .parts
        .iter()
        .map(|p| {
            let pw = crate::model::part_widget(parent, p);
            let frame = part_frame(parent, &vm.body, p);
            let data = results.get(&pw.id).cloned();
            let mut v = widget_vm(doc, &pw, frame, data);
            let e = crate::model::effective(&doc.theme, &pw);
            v.eff = Some(serde_json::json!({"opts": e.opts, "style": e.style}));
            v
        })
        .collect()
}
