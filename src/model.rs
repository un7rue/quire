//! The document model shared by templates (`.rtpl`) and reports (`.rpt`).
//!
//! A template and a report are the same structure: a report is a template
//! snapshot that an end user has filled with parameters, comments and their
//! own widget arrangement. Keeping one shape means a report opens anywhere,
//! even without the template file it came from.

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

pub const FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    #[default]
    Template,
    Report,
    /// A theme being designed in the theme studio: its pages show every
    /// design view with sample data, and edits change the theme.
    Theme,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Doc {
    pub format: u32,
    pub kind: DocKind,
    pub meta: Meta,
    pub page: PageSetup,
    pub theme: Theme,
    pub params: Params,
    pub param_defs: Vec<ParamDef>,
    /// Databases the developer connected; their tables become data sources.
    pub connections: Vec<Connection>,
    pub datasets: Vec<Dataset>,
    pub pages: Vec<Page>,
    pub catalog: Vec<CatalogItem>,
    pub policy: Policy,
    /// Font files carried inside the document so it renders the same anywhere.
    pub fonts: Vec<EmbeddedFont>,
    /// How the fit engine treats edits (reports default to auto-arrange).
    pub arrange: crate::layout::Options,
    pub next_id: u64,
    /// Number, date and currency formats ("" fields follow the theme's language).
    pub formats: Formats,
    /// Icons the designer uploaded (logos), carried inside the document.
    pub icons: Vec<IconFile>,
    /// The designer's own `{{name}}` placeholders.
    pub variables: Vec<Variable>,
    /// The source systems the signed-in person may see (comma-joined, "" = all):
    /// queries read it as `:access_src`. Set from the session, never saved or sent.
    #[serde(skip)]
    pub access_src: String,
    /// The widget studio: this document shows one widget on sample data, edited for the widget library
    /// (the widget type's id, or "new" for a built-in one being made into your own).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub widget_studio: String,
    /// The sample content the widget studio put under the widget (left out when it is saved as a type).
    #[serde(skip)]
    pub studio_sample: Json,
    /// Linked widgets (`opts.follows` = a table): while that table continues
    /// onto more pages, it grows into their rows here (table id → extra rows)…
    #[serde(skip)]
    pub grow: std::collections::HashMap<String, u32>,
    /// …and they are shown after its last rows instead (their mirrors, `opts.mirror`).
    #[serde(skip)]
    pub moved: std::collections::HashSet<String>,
    /// A table piece that shows at most this many rows, so its last rows go on to the next
    /// page with the widgets linked to its end when they no longer fit under it (piece id → rows).
    #[serde(skip)]
    pub keep: std::collections::HashMap<String, u32>,
}

/// A placeholder the designer defines: fixed text, the first cell of an SQL
/// query, or a value from a dataset (`Source.field|agg`), optionally formatted.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Variable {
    pub name: String,
    pub label: String,
    /// "text", "sql" or "data"
    pub kind: String,
    pub value: String,
    /// "" (as it comes), "number", "currency", "percent", "date"
    pub format: String,
    pub description: String,
}

/// An uploaded icon: an SVG or a picture, as a data URL.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct IconFile {
    pub id: String,
    pub name: String,
    pub href: String,
}

impl Formats {
    /// These formats, with empty choices taken from `base` (the app settings),
    /// and separators that can't clash: a comma for thousands makes the decimal
    /// mark a dot, and the other way round.
    pub fn over(&self, base: &Formats, locale: &str) -> Formats {
        let pick = |a: &String, b: &String| if a.is_empty() { b.clone() } else { a.clone() };
        let mut f = Formats {
            date: pick(&self.date, &base.date),
            thousands: pick(&self.thousands, &base.thousands),
            decimal: pick(&self.decimal, &base.decimal),
            currency: pick(&self.currency, &base.currency),
            currency_pos: pick(&self.currency_pos, &base.currency_pos),
            negative: pick(&self.negative, &base.negative),
        };
        let lang_decimal = if locale.starts_with("ru") || locale.starts_with("de") || locale.starts_with("fr") { "," } else { "." };
        let decimal = if f.decimal.is_empty() { lang_decimal } else { f.decimal.as_str() };
        if f.thousands == decimal {
            f.decimal = if decimal == "," { ".".into() } else { ",".into() };
        }
        f
    }
}

/// How numbers, dates and money read. An empty field follows the language.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Formats {
    /// "dd.mm.yyyy", "dd/mm/yyyy", "mm/dd/yyyy", "yyyy-mm-dd", "d mmm yyyy", "mmm d, yyyy"
    pub date: String,
    /// Thousands separator: "space", "thin", ",", ".", "'", "none"
    pub thousands: String,
    /// Decimal separator: "," or "."
    pub decimal: String,
    /// Currency symbol (overrides the theme's)
    pub currency: String,
    /// "before" or "after" the amount
    pub currency_pos: String,
    /// Negative amounts: "minus" (−1 200) or "paren" ((1 200))
    pub negative: String,
}

impl Default for Doc {
    fn default() -> Self {
        let mut d = Doc {
            format: FORMAT,
            kind: DocKind::Template,
            meta: Meta::default(),
            page: PageSetup::default(),
            theme: Theme::default(),
            params: Params::default(),
            param_defs: Vec::new(),
            connections: Vec::new(),
            datasets: Vec::new(),
            pages: Vec::new(),
            catalog: Vec::new(),
            policy: Policy::default(),
            fonts: Vec::new(),
            arrange: crate::layout::Options { auto: true, fill: true },
            next_id: 1,
            formats: Formats::default(),
            icons: Vec::new(),
            variables: Vec::new(),
            access_src: String::new(),
            widget_studio: String::new(),
            studio_sample: Json::Null,
            grow: Default::default(),
            keep: Default::default(),
            moved: Default::default(),
        };
        let id = d.new_id("p");
        d.pages.push(Page { id, title: "Page 1".into(), widgets: Vec::new(), auto: false, section: String::new(), plain: false, ..Default::default() });
        d
    }
}

impl Doc {
    pub fn new_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn page_index(&self, id: &str) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    /// A composite's part by its full id ("parent::part"): page, widget and part index.
    pub fn find_part(&self, id: &str) -> Option<(usize, usize, usize)> {
        let (parent, part) = id.split_once("::")?;
        let (pi, wi) = self.find_widget(parent)?;
        let k = self.pages[pi].widgets[wi].parts.iter().position(|p| p.id == part)?;
        Some((pi, wi, k))
    }

    pub fn find_widget(&self, id: &str) -> Option<(usize, usize)> {
        for (pi, p) in self.pages.iter().enumerate() {
            if let Some(wi) = p.widgets.iter().position(|w| w.id == id) {
                return Some((pi, wi));
            }
        }
        None
    }

    pub fn dataset(&self, id: &str) -> Option<&Dataset> {
        self.datasets.iter().find(|d| d.id == id || d.name == id)
    }

    /// Fresh ids for a widget copied from the catalog or another page.
    pub fn instantiate(&mut self, mut w: Widget) -> Widget {
        w.id = self.new_id("w");
        w
    }

    /// Ensures ids are unique and counters are ahead of every id in the file.
    pub fn normalize(&mut self) {
        let mut max = self.next_id;
        let mut bump = |id: &str| {
            let n: u64 = id.trim_start_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(0);
            if n >= max {
                max = n + 1;
            }
        };
        for p in &self.pages {
            bump(&p.id);
            for w in &p.widgets {
                bump(&w.id);
            }
        }
        for d in &self.datasets {
            bump(&d.id);
        }
        for c in &self.catalog {
            bump(&c.id);
        }
        self.next_id = max;
        if self.pages.is_empty() {
            let id = self.new_id("p");
            self.pages.push(Page { id, title: "Page 1".into(), widgets: Vec::new(), auto: false, section: String::new(), plain: false, ..Default::default() });
        }
        for c in &mut self.catalog {
            c.widget.ensure_ids();
        }
        // dot scales are a bar chart type now ("Range"), not a widget of their own
        let renamed = self.theme.upgrade();
        let ws = self.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()).chain(self.catalog.iter_mut().map(|c| &mut c.widget));
        for w in ws {
            upgrade_scale(w, &renamed);
        }
        // older page setups kept the footer as three strings and tabs as a switch
        let pg = &mut self.page;
        if let Some(t) = pg.footer_left.take() {
            pg.footer.left.text = t;
        }
        if let Some(t) = pg.footer_center.take() {
            pg.footer.center.text = t;
        }
        if let Some(t) = pg.footer_right.take() {
            pg.footer.right.text = t;
        }
        if let Some(tabs) = pg.header_tabs.take() {
            pg.header.show = tabs;
            pg.header.tabs = if tabs { "left".into() } else { "none".into() };
        }
        let cols = self.page.cols.max(1);
        for p in &mut self.pages {
            for w in &mut p.widgets {
                w.ensure_ids();
                w.w = w.w.clamp(1, cols);
                w.h = w.h.max(1);
                w.x = w.x.min(cols - w.w);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub organization: String,
    pub description: String,
    /// Title and version of the template a report was created from.
    pub template_title: String,
    pub template_version: u32,
    pub created: String,
    /// The published report this template is (Reports area), "" until published.
    pub report_id: String,
}

impl Default for Meta {
    fn default() -> Self {
        Meta {
            title: "Untitled report".into(),
            subtitle: String::new(),
            author: String::new(),
            organization: String::new(),
            description: String::new(),
            template_title: String::new(),
            template_version: 1,
            created: Local::now().format("%Y-%m-%d").to_string(),
            report_id: String::new(),
        }
    }
}

// ------------------------------------------------------------------ page

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PageSetup {
    /// "A4", "A3", "Letter" or "Slide" (16:9, 13.33 × 7.5 in).
    pub size: String,
    pub landscape: bool,
    /// Margin around the grid, in points.
    pub margin: f64,
    pub cols: u32,
    pub rows: u32,
    /// Gutter between grid cells, in points.
    pub gap: f64,
    /// The band above the grid (logo, title, section tabs…).
    pub header: Band,
    /// The band below the grid (page numbers, contacts…).
    #[serde(deserialize_with = "band_or_switch")]
    pub footer: Band,
    // older files: a footer switch, three footer texts and a tabs switch
    #[serde(skip_serializing)]
    pub footer_left: Option<String>,
    #[serde(skip_serializing)]
    pub footer_center: Option<String>,
    #[serde(skip_serializing)]
    pub footer_right: Option<String>,
    #[serde(skip_serializing)]
    pub header_tabs: Option<bool>,
    /// The bands' own look (fill, line, line_color, tabs of each), kept while a theme paints them:
    /// what they go back to when another theme comes on.
    #[serde(skip_serializing_if = "Json::is_null")]
    pub own_bands: Json,
}

/// Older files stored the footer as an on/off switch.
fn band_or_switch<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Band, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either {
        Switch(bool),
        Band(Box<Band>),
    }
    Ok(match Either::deserialize(d)? {
        Either::Switch(on) => Band { show: on, ..Band::footer() },
        Either::Band(b) => *b,
    })
}

/// One slot of a header or footer: text (with placeholders) and/or an image.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub text: String,
    /// An image as a data URL (a logo).
    pub image: String,
    pub image_h: f64,
    /// Width ÷ height of the image, measured when it is chosen.
    pub image_ratio: f64,
    /// Font overrides on top of the "footer" text role.
    pub font: FontSpec,
}

/// A header or footer band.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Band {
    pub show: bool,
    /// Height in points.
    pub height: f64,
    /// Background colour ("" = none).
    pub fill: String,
    /// "none", "top" or "bottom" divider line.
    pub line: String,
    pub line_color: String,
    /// Section tabs: "none", "left", "center", "right" or "full".
    pub tabs: String,
    pub left: Slot,
    pub center: Slot,
    pub right: Slot,
}

impl Default for Band {
    fn default() -> Self {
        Band {
            show: false,
            height: 30.0,
            fill: String::new(),
            line: "none".into(),
            line_color: String::new(),
            tabs: "none".into(),
            left: Slot::default(),
            center: Slot::default(),
            right: Slot::default(),
        }
    }
}

impl Band {
    fn footer() -> Band {
        Band {
            show: true,
            height: FOOTER_H,
            line: "none".into(),
            left: Slot { text: "{{title}} · {{period}}".into(), ..Default::default() },
            right: Slot { text: "{{page}} / {{pages}}".into(), ..Default::default() },
            ..Default::default()
        }
    }
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            size: "A4".into(),
            landscape: true,
            margin: 28.0,
            cols: 12,
            rows: 12,
            gap: 8.0,
            header: Band::default(),
            footer: Band::footer(),
            footer_left: None,
            footer_center: None,
            footer_right: None,
            header_tabs: None,
            own_bands: Json::Null,
        }
    }
}

pub const FOOTER_H: f64 = 18.0;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl PageSetup {
    /// Page size in points.
    pub fn dims(&self) -> (f64, f64) {
        let (w, h) = match self.size.as_str() {
            "A3" => (841.89, 1190.55),
            "Letter" => (612.0, 792.0),
            "Slide" => (540.0, 960.0),
            _ => (595.28, 841.89),
        };
        if self.landscape {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// The area the grid occupies.
    pub fn content(&self) -> Frame {
        let (pw, ph) = self.dims();
        let foot = if self.footer.show { self.footer.height.max(8.0) } else { 0.0 };
        let head = if self.header.show { self.header.height.max(8.0) } else { 0.0 };
        Frame { x: self.margin, y: self.margin + head, w: pw - 2.0 * self.margin, h: ph - 2.0 * self.margin - foot - head }
    }

    pub fn cell(&self) -> (f64, f64) {
        let c = self.content();
        let cols = self.cols.max(1) as f64;
        let rows = self.rows.max(1) as f64;
        ((c.w - self.gap * (cols - 1.0)) / cols, (c.h - self.gap * (rows - 1.0)) / rows)
    }

    /// Grid rectangle → page rectangle in points.
    pub fn frame(&self, x: u32, y: u32, w: u32, h: u32) -> Frame {
        let c = self.content();
        let (cw, ch) = self.cell();
        Frame {
            x: c.x + x as f64 * (cw + self.gap),
            y: c.y + y as f64 * (ch + self.gap),
            w: w as f64 * cw + (w.max(1) - 1) as f64 * self.gap,
            h: h as f64 * ch + (h.max(1) - 1) as f64 * self.gap,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub widgets: Vec<Widget>,
    /// Created automatically for a table that continues; removed when no longer needed.
    pub auto: bool,
    /// Section the page belongs to (section tabs, table of contents).
    pub section: String,
    /// No section tabs or footer (a cover page).
    pub plain: bool,
    /// Pages sharing a group show each other in a heading's page navigation
    /// (e.g. "Overview · Holdings 1 · Holdings 2" for one account).
    pub group: String,
    /// Short name in page navigation (defaults to the title).
    pub label: String,
    /// Colour of a thin inset frame drawn around the page ("" = none).
    pub frame: String,
    /// One line about the page, shown under its title in the contents.
    pub description: String,
    /// Left out of the table of contents.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub toc_hidden: bool,
    /// Shown only when this text (usually a data placeholder, e.g.
    /// `{{data:port.has_du}}`) comes out non-empty and not 0 — for pages
    /// some clients or periods have nothing for. "" = always shown.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub show_if: String,
    /// Repeat this page (with the other pages of its group) once per row of a data source.
    #[serde(skip_serializing_if = "Repeat::is_off")]
    pub repeat: Repeat,
    /// On a copy made for a repeated page: the page it copies.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub repeat_of: String,
    /// The row a repeated page (or its copy) shows: column → value, for `{{row.column}}`.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub row: std::collections::BTreeMap<String, String>,
    /// A designed continuation page whose table ended on an earlier page: left out.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub piece_empty: bool,
    /// Which copy this is (0 = the designed page) and how many there are.
    #[serde(skip_serializing_if = "is_zero")]
    pub copy: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub copies: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// How a page repeats: once per row of `dataset` (only rows whose `field` is
/// one of `values`, when set), each copy named by its `label` column.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Repeat {
    pub dataset: String,
    pub field: String,
    pub values: Vec<String>,
    pub label: String,
}

impl Repeat {
    pub fn is_off(&self) -> bool {
        self.dataset.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Widget {
    pub id: String,
    /// kpi, bar, line, area, pie, table, text, heading, comment, divider, image
    pub kind: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub title: String,
    pub subtitle: String,
    /// Locked widgets can't be moved, edited or removed by end users.
    pub locked: bool,
    pub binding: Option<Binding>,
    /// Look & feel: color, background, border, font size… (renderer-defined).
    pub style: Json,
    /// Kind-specific options: chart variant, text, number formats…
    pub opts: Json,
    /// Catalog entry this widget was created from, if any.
    pub catalog: Option<String>,
    /// The theme's design view this widget uses (its own opts/style win over it).
    pub variant: String,
    /// The widget type it was made from (the designer's own, from the library): its settings sit
    /// under the widget's own, over the design view.
    #[serde(rename = "type", skip_serializing_if = "String::is_empty")]
    pub wtype: String,
    /// A composite widget's parts: widgets of their own (a figure, a mini chart, text, an icon…) placed on a
    /// small grid inside its card (`opts.grid`: cols × rows), reading its data source and filters. A part is
    /// addressed as "parent::part" everywhere (selection, edits, its data and drawing).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Widget>,
}

/// A designer's own widget type: a configured widget saved under a name, reused in any template.
/// Widgets made from it follow it (its settings under their own); each template keeps a copy of
/// the types it uses, so a published report changes only when it is published again.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetType {
    pub id: String,
    pub name: String,
    pub description: String,
    /// An icon name for the palette.
    pub icon: String,
    pub kind: String,
    /// The design view it starts from ("" = none).
    pub variant: String,
    pub opts: Json,
    pub style: Json,
    /// What a new widget of the type starts with: its data, title and size.
    pub binding: Option<Binding>,
    pub title: String,
    pub subtitle: String,
    pub w: u32,
    pub h: u32,
    pub version: u32,
    pub by: String,
    pub updated: String,
    /// A composite type's parts: every widget of the type takes them (when it is made, and when the type changes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Widget>,
}

impl Default for Widget {
    fn default() -> Self {
        Widget {
            id: String::new(),
            kind: "text".into(),
            x: 0,
            y: 0,
            w: 4,
            h: 2,
            title: String::new(),
            subtitle: String::new(),
            locked: false,
            binding: None,
            style: json!({}),
            opts: json!({}),
            catalog: None,
            variant: String::new(),
            wtype: String::new(),
            parts: Vec::new(),
        }
    }
}

impl Widget {
    /// Gives every measure a stable id; older files used the position ("m0", "m1"…).
    pub fn ensure_ids(&mut self) {
        let Some(b) = self.binding.as_mut() else { return };
        let mut used: Vec<String> = b.measures.iter().map(|m| m.id.clone()).filter(|i| !i.is_empty()).collect();
        for i in 0..b.measures.len() {
            if !b.measures[i].id.is_empty() {
                continue;
            }
            let mut n = i;
            while used.contains(&format!("m{n}")) {
                n += 1;
            }
            b.measures[i].id = format!("m{n}");
            used.push(b.measures[i].id.clone());
        }
    }

    pub fn opt_str(&self, key: &str) -> Option<&str> {
        self.opts.get(key).and_then(|v| v.as_str())
    }

    pub fn style_f64(&self, key: &str) -> Option<f64> {
        self.style.get(key).and_then(|v| v.as_f64())
    }

    pub fn opt_f64(&self, key: &str) -> Option<f64> {
        self.opts.get(key).and_then(|v| v.as_f64())
    }

    pub fn opt_bool(&self, key: &str) -> Option<bool> {
        self.opts.get(key).and_then(|v| v.as_bool())
    }

}

/// What a widget shows: an aggregation of one dataset.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Binding {
    pub dataset: String,
    /// Group-by fields. A date field may carry a grain: "date:month".
    pub dims: Vec<String>,
    pub measures: Vec<Measure>,
    /// Which slice of time to show: period, ytd, trailing3/6/12/24, all.
    pub window: String,
    pub filters: Vec<Filter>,
    /// "", "value_desc", "value_asc", "dim_asc", "dim_desc".
    pub sort: String,
    /// Keep the first N groups (0 = all).
    pub limit: u32,
    /// Fold groups beyond `limit` into an "Other" row.
    pub others: bool,
    /// "" (summary: group and aggregate) or "rows" (detail: list rows as they are).
    pub mode: String,
    /// Fields shown in detail mode, in order.
    pub columns: Vec<String>,
    /// Tables: spread this field across the columns (a pivot), e.g. "date:month".
    pub pivot: String,
    /// A SQL condition the rows must meet, e.g. `amount > 1e6 AND region <> 'West'`.
    pub where_sql: String,
}

impl Default for Binding {
    fn default() -> Self {
        Binding {
            dataset: String::new(),
            dims: Vec::new(),
            measures: Vec::new(),
            window: "period".into(),
            filters: Vec::new(),
            sort: String::new(),
            limit: 0,
            others: false,
            mode: String::new(),
            columns: Vec::new(),
            pivot: String::new(),
            where_sql: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Measure {
    /// Stable column key, so styles and order follow the value when it moves.
    pub id: String,
    pub field: String,
    /// sum, avg, count, distinct, min, max, first, last
    pub agg: String,
    pub label: String,
    /// number, currency, percent, compact
    pub format: String,
    /// Optional ratio: this measure divided by another measure's field (e.g. margin %).
    pub divide_by: Option<String>,
    /// "" (the value), "prev" (comparison period), "delta" (change), "pct" (% change),
    /// "share" (of the column total), "rank", "running" (cumulative) or "trend" (12-month sparkline).
    pub calc: String,
    /// Bar charts: draw this measure as a line over the bars ("line").
    pub chart: Option<String>,
    /// A fixed colour for this measure's series.
    pub color: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub field: String,
    /// =, !=, >, >=, <, <=, in, contains
    pub op: String,
    /// Literal, or `{{param}}` to follow a report parameter.
    pub value: String,
}

// ------------------------------------------------------------------ theme

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub palette: Vec<String>,
    pub accent: String,
    pub ink: String,
    pub muted: String,
    pub grid: String,
    pub page_bg: String,
    pub card_bg: String,
    /// "plain", "outlined" or "filled".
    pub card: String,
    /// Card outline colour and width (outlined cards; "" = automatic).
    pub card_border: String,
    pub card_border_w: f64,
    /// Inner padding of cards (0: the standard 10pt).
    pub card_pad: f64,
    pub radius: f64,
    pub font_scale: f64,
    pub good: String,
    pub bad: String,
    /// Symbol for currency-formatted measures.
    pub currency: String,
    /// Base font family for the whole report.
    pub font_family: String,
    /// Language and number format: "en-US", "ru-RU", "de-DE"…
    pub locale: String,
    /// Chart look: "clean" (flat marks) or "soft" (tinted, outlined, annotated).
    pub chart_look: String,
    /// Visual defaults for charts, by kind ("all", "bar", "line", "pie"…).
    pub viz: serde_json::Value,
    /// Per-role font settings: heading, heading_sub, title, subtitle, body,
    /// caption, kpi, table_head, table_cell, chart, footer.
    pub fonts: BTreeMap<String, FontSpec>,
    /// The theme's name (shown in the theme library).
    pub name: String,
    /// Design views by widget kind; the first of a kind is the default for new widgets.
    pub styles: BTreeMap<String, Vec<StyleVariant>>,
    /// Named colours ("dividend", "gainTint"…). Anywhere a colour is written as
    /// `$name` it comes from here (or from good, bad, accent, ink, muted, grid,
    /// page, card), so switching themes recolours everything.
    pub tokens: BTreeMap<String, String>,
    /// The look a restyle from a picture built this theme from (colours, typefaces, shape),
    /// so the next restyle starts from it. Null for other themes.
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub look: Json,
    /// The widget types the document's widgets use (copies from the library).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub widget_types: BTreeMap<String, WidgetType>,
    /// Section tabs in the page header: {style: pill|box|underline|text, fill, color, weight, upper}
    /// (null: dark pills).
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub tabs: Json,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            palette: ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            accent: "#2a78d6".into(),
            ink: "#0b0b0b".into(),
            muted: "#6b6a65".into(),
            grid: "#e1e0d9".into(),
            page_bg: "#ffffff".into(),
            card_bg: "#fcfcfb".into(),
            card: "outlined".into(),
            card_border: String::new(),
            card_border_w: 0.75,
            card_pad: 0.0,
            radius: 6.0,
            font_scale: 1.0,
            good: "#006300".into(),
            bad: "#d03b3b".into(),
            currency: "$".into(),
            font_family: "Inter".into(),
            locale: "en-US".into(),
            chart_look: "clean".into(),
            viz: serde_json::json!({}),
            fonts: BTreeMap::new(),
            name: String::new(),
            styles: BTreeMap::new(),
            tokens: BTreeMap::new(),
            look: Json::Null,
            tabs: Json::Null,
            widget_types: BTreeMap::new(),
        }
    }
}

/// A dot-scale widget (an older kind) as a bar chart of type "Range".
pub fn upgrade_scale(w: &mut Widget, renamed: &BTreeMap<String, String>) {
    if w.kind != "scale" {
        return;
    }
    w.kind = "bar".into();
    if !w.opts.is_object() {
        w.opts = serde_json::json!({});
    }
    w.opts["orientation"] = "scale".into();
    if let Some(id) = renamed.get(&w.variant) {
        w.variant = id.clone();
    }
}

impl Theme {
    /// Moves the theme's dot-scale design views to the bar chart (as Range
    /// views). Returns the views that got a new id (old id → new id).
    pub fn upgrade(&mut self) -> BTreeMap<String, String> {
        self.heal_colours();
        let mut renamed = BTreeMap::new();
        let Some(views) = self.styles.remove("scale") else { return renamed };
        let bars = self.styles.entry("bar".into()).or_default();
        for mut v in views {
            if bars.iter().any(|b| b.id == v.id) {
                let id = format!("range-{}", v.id);
                renamed.insert(v.id.clone(), id.clone());
                v.id = id;
            }
            if !v.opts.is_object() {
                v.opts = serde_json::json!({});
            }
            v.opts["orientation"] = "scale".into();
            bars.push(v);
        }
        if let Some(d) = self.viz.as_object_mut().and_then(|o| o.remove("scale")) {
            // the old kind's defaults now only apply to range bars, so drop them
            let _ = d;
        }
        renamed
    }
}

impl Theme {
    /// Colour pairs that must read, mended where a theme (often a restyle's) broke them: the icon
    /// on its chip, and the gain / loss tints, which stand behind green and red marks and so keep
    /// the hue of good / bad (a pink "gain" chip reads as a loss).
    pub fn heal_colours(&mut self) {
        use crate::ai_restyle::contrast;
        let hex = |s: &str| s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit());
        let get = |t: &BTreeMap<String, String>, k: &str| t.get(k).cloned().filter(|v| hex(v));
        if let (Some(chip), Some(ink)) = (get(&self.tokens, "chip"), get(&self.tokens, "iconInk")) {
            if contrast(&chip, &ink) < 3.0 {
                let alt = if contrast(&chip, "#000000") >= contrast(&chip, "#ffffff") { "#000000" } else { "#ffffff" };
                self.tokens.insert("iconInk".into(), alt.into());
            }
        }
        let paper = if hex(&self.page_bg) { self.page_bg.clone() } else { "#ffffff".into() };
        for (tint, base) in [("gainTint", &self.good), ("lossTint", &self.bad)] {
            let (Some(t), true) = (get(&self.tokens, tint), hex(base)) else { continue };
            if !tint_fits(&t, base) {
                self.tokens.insert(tint.into(), crate::ai_restyle::mix(base, &paper, 0.86));
            }
        }
    }
}

/// A pale fill fits behind a gain / loss colour: its hue is close (a near-grey fill fits any)
/// and the colour still reads on it.
pub fn tint_fits(tint: &str, base: &str) -> bool {
    use crate::ai_restyle::{contrast, hue};
    let near = match (hue(tint), hue(base)) {
        (Some(a), Some(b)) => { let d = (a - b).abs() % 360.0; d.min(360.0 - d) <= 50.0 }
        _ => true,
    };
    near && contrast(tint, base) >= 2.0
}

/// A named design view of one widget kind ("Hero KPI", "Toned card",
/// "Grouped table"…): the options and style every widget using it starts from.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StyleVariant {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Sample data the theme studio shows it with ("trend", "categories", "signed"…).
    pub sample: String,
    pub opts: Json,
    pub style: Json,
}

/// RFC 7386 merge patch.
pub fn merge_json(target: &mut Json, patch: &Json) {
    match patch {
        Json::Object(p) => {
            if !target.is_object() {
                *target = json!({});
            }
            let t = target.as_object_mut().unwrap();
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge_json(t.entry(k.clone()).or_insert(Json::Null), v);
                }
            }
        }
        other => *target = other.clone(),
    }
}

impl Theme {
    /// A colour, with `$name` references resolved against the theme.
    pub fn color(&self, s: &str) -> String {
        let Some(name) = s.strip_prefix('$') else { return s.to_string() };
        match name {
            "good" => self.good.clone(),
            "bad" => self.bad.clone(),
            "accent" => self.accent.clone(),
            "ink" => self.ink.clone(),
            "muted" => self.muted.clone(),
            "grid" => self.grid.clone(),
            "page" => self.page_bg.clone(),
            "card" => self.card_bg.clone(),
            "cardBorder" => if self.card_border.is_empty() { self.grid.clone() } else { self.card_border.clone() },
            // the series colours by their place: $series1 … $series8
            n if n.starts_with("series") && n[6..].parse::<usize>().is_ok() => {
                n[6..].parse::<usize>().ok().and_then(|i| i.checked_sub(1)).and_then(|i| self.palette.get(i)).cloned().unwrap_or_else(|| "#888888".into())
            }
            // a derived colour, as its hex or by a main colour's name ($accent …), which it then follows
            _ => match self.tokens.get(name) {
                Some(v) if v.starts_with('$') && !self.tokens.contains_key(&v[1..]) => self.color(v),
                Some(v) if !v.starts_with('$') => v.clone(),
                _ => "#888888".into(),
            },
        }
    }

    /// A derived colour (`tokens`) as drawn: its name resolved; None when unset or empty.
    pub fn token(&self, k: &str) -> Option<String> {
        self.tokens.get(k).filter(|v| !v.is_empty()).map(|_| self.color(&format!("${k}"))).filter(|c| c.starts_with('#'))
    }

    /// The theme's named colours as they are now: the main ones (in the Colours tab's names) and the series.
    pub(crate) fn named_colours(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = ["accent", "ink", "muted", "good", "bad", "page", "card", "grid", "cardBorder"]
            .iter().map(|n| (format!("${n}"), self.color(&format!("${n}")).to_lowercase())).collect();
        v.extend(self.palette.iter().enumerate().map(|(i, c)| (format!("$series{}", i + 1), c.to_lowercase())));
        v
    }

    /// Colours that repeat one of the theme's named colours point to it by name (`$accent`, `$series2`…), so they
    /// follow it: the derived colours, the design views, the text roles, chart defaults and the page bands. The
    /// first name wins where two share a colour. Nothing is drawn differently. Returns how many were linked.
    pub fn link_colours(&mut self) -> usize {
        let named = self.named_colours();
        let name_of = |c: &str| -> Option<String> {
            let c = c.trim().to_lowercase();
            (c.len() == 7 && c.starts_with('#')).then(|| named.iter().find(|(_, h)| *h == c).map(|(n, _)| n.clone())).flatten()
        };
        fn walk(v: &mut Json, name_of: &dyn Fn(&str) -> Option<String>, n: &mut usize) {
            match v {
                Json::String(s) => if let Some(t) = name_of(s) { *s = t; *n += 1; },
                Json::Array(a) => a.iter_mut().for_each(|x| walk(x, name_of, n)),
                Json::Object(m) => m.values_mut().for_each(|x| walk(x, name_of, n)),
                _ => {}
            }
        }
        let mut n = 0;
        for v in self.tokens.values_mut() {
            if let Some(t) = name_of(v) { *v = t; n += 1; }
        }
        for list in self.styles.values_mut() {
            for sv in list.iter_mut() {
                walk(&mut sv.opts, &name_of, &mut n);
                walk(&mut sv.style, &name_of, &mut n);
            }
        }
        for f in self.fonts.values_mut() {
            if let Some(t) = f.color.as_deref().and_then(name_of) { f.color = Some(t); n += 1; }
        }
        walk(&mut self.viz, &name_of, &mut n);
        if let Some(p) = self.look.get_mut("page") {
            walk(p, &name_of, &mut n);
        }
        n
    }

    pub fn variant(&self, kind: &str, id: &str) -> Option<&StyleVariant> {
        if id.is_empty() {
            return None;
        }
        self.styles.get(kind).and_then(|v| v.iter().find(|s| s.id == id))
    }
}

fn is_token(s: &str) -> bool {
    s.len() > 1 && s.starts_with('$') && s[1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn has_tokens(v: &Json) -> bool {
    match v {
        Json::String(s) => is_token(s),
        Json::Array(a) => a.iter().any(has_tokens),
        Json::Object(o) => o.values().any(has_tokens),
        _ => false,
    }
}

/// Replaces `$name` colour references with the theme's colours.
pub fn resolve_tokens(theme: &Theme, v: &mut Json) {
    match v {
        Json::String(s) if is_token(s) => *s = theme.color(s),
        Json::Array(a) => a.iter_mut().for_each(|x| resolve_tokens(theme, x)),
        Json::Object(o) => o.values_mut().for_each(|x| resolve_tokens(theme, x)),
        _ => {}
    }
}

/// A vertical divider drawn in the gap before its column (`opts.gap`): it takes no room on the grid, so the
/// widgets either side may cover its column, and the fit engine leaves it where it is.
pub fn in_gap(theme: &Theme, w: &Widget) -> bool {
    w.kind == "divider" && {
        let e = effective(theme, w);
        e.opt_bool("vertical") == Some(true) && e.opt_bool("gap") == Some(true)
    }
}

impl Doc {
    /// Charts of monthly values and their running total — two series, columns and a line, with no settings of
    /// their own per series yet — read best on two axes: the total on the right axis, labelled at its last value,
    /// and every month's column labelled outside it. A chart whose series were set by hand is left as it is.
    /// Returns how many charts took it.
    pub fn split_series_axes(&mut self) -> usize {
        let mut n = 0;
        for w in self.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()) {
            let Some(b) = w.binding.as_ref().filter(|_| w.kind == "bar") else { continue };
            if b.measures.len() != 2 || !b.dims.len().eq(&1) {
                continue;
            }
            let is_line = |m: &Measure| m.chart.as_deref() == Some("line");
            let (Some(col), Some(line)) = (b.measures.iter().find(|m| !is_line(m)), b.measures.iter().find(|m| is_line(m))) else { continue };
            let has_own = w.opts.pointer("/viz/seriesOpts").and_then(|v| v.as_object()).is_some_and(|o| !o.is_empty());
            if has_own {
                continue;
            }
            let so = json!({ col.id.clone(): {"labels": "all", "labelPos": "outside"}, line.id.clone(): {"axis": "right", "labels": "last"} });
            if !w.opts.is_object() { w.opts = json!({}); }
            if !w.opts["viz"].is_object() { w.opts["viz"] = json!({}); }
            w.opts["viz"]["seriesOpts"] = so;
            n += 1;
        }
        n
    }

    /// Vertical dividers between two halves of a page go: the halves are told apart by the gap between them.
    /// A divider in a column of its own gives that column to the side with fewer columns (where nothing else is
    /// in the way), so the halves come out even (12 and 12); one already in the gap simply goes. A vertical
    /// divider with nothing beside it stays. Nothing else moves. Returns how many dividers went.
    pub fn remove_split_dividers(&mut self) -> usize {
        let cols = self.page.cols.max(1);
        let mut n = 0;
        for pi in 0..self.pages.len() {
            let mut di = 0;
            while di < self.pages[pi].widgets.len() {
                let d = self.pages[pi].widgets[di].clone();
                let vertical = d.kind == "divider" && effective(&self.theme, &d).opt_bool("vertical") == Some(true);
                let gap = vertical && in_gap(&self.theme, &d);
                if !vertical || (!gap && d.w != 1) {
                    di += 1;
                    continue;
                }
                let theme = &self.theme;
                let ws = &mut self.pages[pi].widgets;
                let rows = |w: &Widget| w.y < d.y + d.h && d.y < w.y + w.h;
                let others = |i: usize, ws: &[Widget]| i != di && rows(&ws[i]) && !in_gap(theme, &ws[i]) && ws[i].kind != "divider";
                // in the gap the halves meet at its column's left edge; in a column of its own, around it
                let (lx, rx) = if gap { (d.x, d.x) } else { (d.x, d.x + 1) };
                let left: Vec<usize> = (0..ws.len()).filter(|&i| others(i, ws) && ws[i].x + ws[i].w == lx).collect();
                let right: Vec<usize> = (0..ws.len()).filter(|&i| others(i, ws) && ws[i].x == rx).collect();
                if left.is_empty() || right.is_empty() {
                    di += 1;
                    continue;
                }
                if !gap {
                    // the narrower side takes the divider's column, where it is free
                    let c = d.x;
                    let take_left = c < cols - c - 1;
                    for &i in if take_left { &left } else { &right } {
                        let w = &ws[i];
                        let free = !ws.iter().enumerate().any(|(j, o)| {
                            j != i && j != di && !in_gap(theme, o) && o.x <= c && c < o.x + o.w && o.y < w.y + w.h && w.y < o.y + o.h
                        });
                        if free {
                            let w = &mut ws[i];
                            if take_left { w.w += 1 } else { w.x -= 1; w.w += 1 }
                        }
                    }
                }
                ws.remove(di);
                n += 1;
            }
        }
        n
    }
}

/// The widget as it renders: its design view's options and style with the
/// widget's own settings on top, and theme colour references resolved.
pub fn effective<'a>(theme: &Theme, w: &'a Widget) -> std::borrow::Cow<'a, Widget> {
    // a widget type: its design view (unless the widget picks another) and its settings, under the widget's own
    let wt = (!w.wtype.is_empty()).then(|| theme.widget_types.get(&w.wtype)).flatten().filter(|t| t.kind == w.kind);
    let view = if w.variant.is_empty() { wt.map(|t| t.variant.as_str()).unwrap_or("") } else { w.variant.as_str() };
    match theme.variant(&w.kind, view) {
        None if wt.is_none() && !has_tokens(&w.opts) && !has_tokens(&w.style) => std::borrow::Cow::Borrowed(w),
        v => {
            let mut e = w.clone();
            if v.is_some() || wt.is_some() {
                let mut opts = v.filter(|v| v.opts.is_object()).map(|v| v.opts.clone()).unwrap_or_else(|| json!({}));
                let mut style = v.filter(|v| v.style.is_object()).map(|v| v.style.clone()).unwrap_or_else(|| json!({}));
                if let Some(t) = wt {
                    if t.opts.is_object() { merge_json(&mut opts, &t.opts); }
                    if t.style.is_object() { merge_json(&mut style, &t.style); }
                }
                merge_json(&mut opts, &w.opts);
                merge_json(&mut style, &w.style);
                e.opts = opts;
                e.style = style;
                e.variant = view.to_string();
            }
            resolve_tokens(theme, &mut e.opts);
            resolve_tokens(theme, &mut e.style);
            std::borrow::Cow::Owned(e)
        }
    }
}

/// Partial font settings; unset fields inherit.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FontSpec {
    pub family: Option<String>,
    pub size: Option<f64>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub color: Option<String>,
    pub upper: Option<bool>,
    /// Letter spacing in em (0.1 = a tenth of the font size between letters).
    pub tracking: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddedFont {
    pub family: String,
    pub file: String,
    /// Base64 of the font file.
    pub data: String,
}

// ------------------------------------------------------------------ params

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    /// month, quarter, year, ytd, custom
    pub grain: String,
    /// Any date inside the reporting period (its end for custom ranges).
    pub date: NaiveDate,
    /// Start of a custom range.
    pub start: Option<NaiveDate>,
    /// previous, year, none
    pub compare: String,
    /// Values of template-defined parameters.
    pub values: BTreeMap<String, String>,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            grain: "month".into(),
            date: Local::now().date_naive(),
            start: None,
            compare: "previous".into(),
            values: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ParamDef {
    pub name: String,
    pub label: String,
    /// Allowed values; empty means free text. "All" is always allowed.
    pub options: Vec<String>,
    pub default: String,
    /// "list" (choices), "text", "number" or "date".
    pub kind: String,
    /// Choices read from a dataset column (its distinct values) instead of `options`.
    pub source: Option<ParamSource>,
    /// Several values at once (stored joined by U+001F).
    pub multi: bool,
    /// The column this parameter filters in every dataset that has it ("" = none:
    /// widgets use `{{name}}` in their own filters, SQL uses `:name`).
    pub field: String,
    /// Only these datasets (ids); empty = every dataset with the column.
    pub datasets: Vec<String>,
    /// For report users: "change" (default), "show" (read only) or "hide".
    pub user: String,
    /// The column it filters in particular sources (dataset id → column); "" leaves
    /// that source unfiltered. Sources not listed use `field` when they have it.
    pub columns: BTreeMap<String, String>,
}

impl ParamDef {
    /// The column this parameter filters in a source, if any.
    pub fn column_in(&self, ds_id: &str) -> Option<&str> {
        if let Some(c) = self.columns.get(ds_id) {
            return (!c.trim().is_empty()).then_some(c.trim());
        }
        if self.field.trim().is_empty() || (!self.datasets.is_empty() && !self.datasets.iter().any(|x| x == ds_id)) {
            return None;
        }
        Some(self.field.trim())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ParamSource {
    pub dataset: String,
    pub field: String,
    /// A column shown for each choice instead of the value itself ("" = the value):
    /// the user picks "Брокерские счета", queries get BROK.
    #[serde(default)]
    pub label: String,
}

/// The values a parameter holds: none means "all".
pub fn param_values(v: &str) -> Vec<&str> {
    v.split('\u{1f}').map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("all")).collect()
}

// ------------------------------------------------------------------ data

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Dataset {
    pub id: String,
    pub name: String,
    pub source: Source,
    /// Date column used for period windows and comparisons.
    pub date_field: Option<String>,
    /// Detected schema, refreshed on load.
    pub fields: Vec<Field>,
    /// Values are snapshots per date (balances, headcount, prices) that don't
    /// add up over time: new values default to the last value instead of a sum.
    pub snapshot: bool,
    /// SQL run over the loaded rows (table `data`; other datasets by name).
    pub transform: String,
    /// Extra columns defined by SQL expressions.
    pub calc_fields: Vec<CalcField>,
    /// A shared source from the Data area: the template keeps a copy that is
    /// refreshed from the Data area when the template opens; it is changed
    /// there, not in the template.
    pub shared: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CalcField {
    pub name: String,
    /// A SQL expression over the dataset's columns, e.g. `amount / 1e6`.
    pub expr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    Csv { path: String },
    Excel { path: String, sheet: String },
    Sqlite { path: String, query: String },
    Postgres { url: String, query: String },
    /// ClickHouse over HTTP: clickhouse://user:password@host:8123/database (clickhouses:// for TLS).
    Clickhouse { url: String, query: String },
    /// DuckDB: a database file, or in-memory (empty path) to query CSV/Parquet files with SQL.
    Duckdb { path: String, query: String },
    Python { code: String },
    Inline { columns: Vec<String>, rows: Vec<Vec<Json>> },
    /// SQL over the other datasets (by name): CTEs, joins, window functions.
    /// `:start`, `:end`, `:prev_start`, `:prev_end` and `:param` are filled in.
    Query { sql: String },
    /// SQL against one of the document's connections (by id), run by that
    /// database: any SELECT with joins, CTEs and window functions.
    Db { connection: String, query: String },
}

/// A database connection: a DuckDB or SQLite file, or a PostgreSQL or ClickHouse server.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Connection {
    pub id: String,
    pub name: String,
    /// "duckdb", "sqlite", "postgres" or "clickhouse"
    pub kind: String,
    /// the database file (DuckDB, SQLite): absolute, `~/…`, or next to the document
    pub path: String,
    /// postgres://user:password@host:5432/db, clickhouse://user:password@host:8123/db
    pub url: String,
}

impl Default for Source {
    fn default() -> Self {
        Source::Inline { columns: Vec::new(), rows: Vec::new() }
    }
}

impl Source {
    /// Whether the source itself takes the period (`:data_start`, `:data_end`) as input,
    /// so it has to be re-queried when the period changes.
    pub fn period_aware(&self) -> bool {
        match self {
            Source::Sqlite { query, .. } | Source::Postgres { query, .. } | Source::Clickhouse { query, .. } | Source::Duckdb { query, .. } => {
                ["start", "end", "data_start", "data_end", "period_start", "period_end"].iter().any(|k| query.contains(&format!(":{k}")))
            }
            Source::Python { .. } => true,
            // a query may use the period and the report's parameters (:client…)
            Source::Db { query, .. } => query.contains(':'),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Field {
    pub name: String,
    /// number, text, date
    pub kind: String,
}

// ------------------------------------------------------------------ catalog & policy

/// A predefined widget end users can add to their reports.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub widget: Widget,
    /// The page it was taken from: a figure that follows a repeated page's row
    /// can go only on that page (and so on each of its copies).
    pub page: String,
}

/// What end users may do with reports made from this template.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub add: bool,
    pub remove: bool,
    pub arrange: bool,
    pub restyle: bool,
    pub comment: bool,
    pub pages: bool,
    pub edit_text: bool,
    /// The period in the report toolbar: "change", "show" (read only) or "hide".
    pub period: String,
    /// The comparison in the report toolbar: "change", "show" or "hide".
    pub compare: String,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { add: true, remove: true, arrange: true, restyle: true, comment: true, pages: true, edit_text: true, period: "change".into(), compare: "change".into() }
    }
}

/// Whether report users may change a filter set to `access` ("" counts as "change").
pub fn can_change(access: &str) -> bool {
    access.is_empty() || access == "change"
}

/// The composite grid's size: `opts.grid` {cols, rows}, 12 × 6 unless set.
pub fn part_grid(w: &Widget) -> (u32, u32) {
    let g = |k: &str, d: u32| w.opts.pointer(&format!("/grid/{k}")).and_then(|v| v.as_u64()).map(|v| (v as u32).clamp(1, 48)).unwrap_or(d);
    (g("cols", 12), g("rows", 6))
}

/// A part as a widget of its own: its full id, and the composite's data source and filters under its own
/// (its own data source wins; filters and SQL conditions add up).
pub fn part_widget(parent: &Widget, p: &Widget) -> Widget {
    let mut w = p.clone();
    w.id = format!("{}::{}", parent.id, p.id);
    w.parts = Vec::new();
    w.locked = parent.locked || p.locked;
    if let Some(pb) = &parent.binding {
        match &mut w.binding {
            Some(b) => {
                if b.dataset.is_empty() {
                    b.dataset = pb.dataset.clone();
                }
                let mut f = pb.filters.clone();
                f.extend(b.filters.drain(..));
                b.filters = f;
                b.where_sql = match (pb.where_sql.trim(), b.where_sql.trim()) {
                    ("", own) => own.to_string(),
                    (shared, "") => shared.to_string(),
                    (shared, own) => format!("({shared}) and ({own})"),
                };
            }
            None => {}
        }
    }
    w
}
