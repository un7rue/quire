//! Themes and the theme studio.
//!
//! A theme is colours, fonts and *design views*: named looks for each widget
//! kind ("Big number" KPI, "Grouped" table, "100% bar"…). Templates pick a
//! theme and widgets pick a design view, keeping only their own overrides.
//!
//! The studio is a document of kind `Theme`: its pages are generated from the
//! theme and show every design view with built-in sample data. Editing a
//! widget there edits its design view; the pages are then regenerated.

use crate::model::*;
use crate::period::ymd;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

/// A theme file (`.qtheme`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeFile {
    pub format: String,
    pub theme: Theme,
    /// Font files the theme uses, so it renders the same anywhere.
    pub fonts: Vec<EmbeddedFont>,
}

pub const THEME_FORMAT: &str = "quire-theme";

/// Every kind a theme can style.
pub const KINDS: [&str; 14] = ["kpi", "table", "bar", "line", "area", "pie", "waterfall", "treemap", "tiles", "heading", "text", "comment", "divider", "toc"];

fn v(id: &str, name: &str, description: &str, sample: &str, opts: Json, style: Json) -> StyleVariant {
    StyleVariant { id: id.into(), name: name.into(), description: description.into(), sample: sample.into(), opts, style }
}

/// Design views every new theme starts with.
pub fn base_styles() -> BTreeMap<String, Vec<StyleVariant>> {
    let mut m = BTreeMap::new();
    m.insert("kpi".into(), vec![
        v("card", "Card", "Value, change and a sparkline in a card", "kpi", json!({}), json!({})),
        v("big", "Big number", "A large figure without a card", "kpi", json!({"viz": {"spark": "none", "delta": false, "compact": "off"}}), json!({"card": false, "fonts": {"kpi": {"size": 34}}})),
        v("toned", "Toned card", "Coloured by the result, with an arrow", "kpi-pct", json!({"viz": {"tone": "sign", "arrow": true, "valueColor": "sign", "signed": true, "spark": "none", "delta": false}}), json!({})),
        v("breakdown", "With breakdown", "The other values listed as rows", "kpi", json!({"viz": {"breakdown": true, "spark": "none", "delta": false, "compact": "off"}}), json!({})),
    ]);
    m.insert("table".into(), vec![
        v("standard", "Standard", "Header, rows and totals", "categories", json!({}), json!({})),
        v("grouped", "Grouped", "Group rows in capitals with a rule", "grouped", json!({"table": {"subtotals": true, "groupUpper": true, "groupCount": true, "groupFill": "none", "groupLine": true, "indent": false}}), json!({})),
        v("list", "List", "No header, light rows", "categories", json!({"totals": false, "table": {"hideHead": true, "lines": "rows"}}), json!({"card": false})),
    ]);
    m.insert("bar".into(), vec![
        v("columns", "Columns", "", "trend", json!({}), json!({})),
        v("combo", "Columns + line", "Bars by sign with a cumulative line", "combo", json!({"viz": {"colorMode": "sign", "labels": "all", "callout": "last", "extremes": true}}), json!({})),
        v("bars", "Ranked bars", "", "ranked", json!({"orientation": "horizontal", "viz": {"rankStyle": "ranked"}}), json!({})),
        v("share", "100% bar", "Shares of one total", "categories", json!({"orientation": "share", "viz": {"legendShare": true}}), json!({})),
        v("range", "Range", "Points on one scale: benchmarks, a percentile", "bench", json!({"orientation": "scale"}), json!({})),
    ]);
    m.insert("line".into(), vec![v("line", "Line", "", "trend-value", json!({}), json!({}))]);
    m.insert("area".into(), vec![v("area", "Area", "", "trend-value", json!({}), json!({}))]);
    m.insert("pie".into(), vec![
        v("donut", "Donut", "", "categories", json!({}), json!({})),
        v("donut-table", "Donut with table", "Legend with amounts and shares", "categories", json!({"viz": {"legendStyle": "table", "legendAmount": true}}), json!({})),
    ]);
    m.insert("waterfall".into(), vec![v("waterfall", "Waterfall", "", "trend", json!({}), json!({}))]);
    m.insert("treemap".into(), vec![v("treemap", "Treemap", "", "categories", json!({}), json!({}))]);
    m.insert("tiles".into(), vec![v("tiles", "Tiles", "", "signed", json!({"viz": {"signed": true}}), json!({}))]);
    m.insert("heading".into(), vec![
        v("page", "Page title", "Serif title with a rule", "", json!({"rule": 1.5}), json!({})),
        v("section", "Section title", "Small capitals", "", json!({}), json!({"fonts": {"heading": {"size": 10, "weight": 800, "upper": true, "tracking": 0.05}}})),
        v("eyebrow", "With eyebrow", "A line of capitals above the title", "", json!({"eyebrow": "Section"}), json!({})),
    ]);
    m.insert("text".into(), vec![
        v("body", "Body", "", "", json!({}), json!({})),
        v("note", "Small note", "", "", json!({}), json!({"fonts": {"body": {"size": 7, "color": "#8a8780"}}})),
    ]);
    m.insert("toc".into(), vec![
        v("list", "List", "Pages under section labels, with dotted leaders", "", json!({}), json!({"card": false, "color": "$accent"})),
        v("chapters", "Chapters", "Numbered sections with icons and page descriptions", "", json!({"toc": {"style": "sections", "cols": 2}}), json!({"card": false})),
    ]);
    m
}

/// A new theme: the given colours and fonts with the base design views added
/// for any kind that has none.
pub fn with_base(mut theme: Theme) -> Theme {
    theme.upgrade();
    for (k, list) in base_styles() {
        theme.styles.entry(k).or_insert(list);
    }
    if theme.name.is_empty() {
        theme.name = "Untitled theme".into();
    }
    theme
}

/// The built-in themes: (id, name).
pub fn builtins() -> Vec<(&'static str, &'static str)> {
    crate::uhnwi_theme::LOOKS.iter().map(|(id, l)| (*id, l.name)).collect()
}

pub fn builtin(id: &str) -> Option<Theme> {
    match id {
        // the plain theme the app starts from (no design views of its own)
        "plain" => Some(with_base(Theme { name: "Plain".into(), ..Theme::default() })),
        // the house theme: its own look, with every widget the studio offers (the base views it doesn't have)
        "quire" => {
            let mut t = crate::uhnwi_theme::theme_from(&crate::uhnwi_theme::QUIRE);
            for (k, list) in base_styles() {
                let have = t.styles.entry(k.clone()).or_default();
                for v in list {
                    // a base view the theme already has under the shared id ("big" is "hero") is that widget
                    let shared = same_widget(&k, &v.id);
                    if !have.iter().any(|x| x.id == v.id || (shared != v.id && x.id == shared)) {
                        have.push(v);
                    }
                }
            }
            t.link_colours();
            Some(t)
        }
        other => crate::uhnwi_theme::LOOKS.iter().find(|(id, _)| *id == other).map(|(_, l)| crate::uhnwi_theme::theme_from(l)),
    }
}

// ------------------------------------------------------------------ sample data

/** The studio's sample words in the theme's language, so previews read like real reports. */
const WORDS: &[(&str, &str, &str)] = &[
    ("Category", "Категория", "Kategorie"),
    ("Overview", "Обзор", "Überblick"),
    ("Portfolio overview", "Обзор портфеля", "Portfolioübersicht"),
    ("Assets, results and structure", "Активы, результат и структура", "Vermögen, Ergebnis und Struktur"),
    ("Key figures", "Ключевые показатели", "Kennzahlen"),
    ("Returns by product and period", "Доходность по продуктам и периодам", "Rendite nach Produkt und Zeitraum"),
    ("Detail", "Детальная информация", "Details"),
    ("Accounts", "Счета", "Konten"),
    ("Balances and movements on each account", "Остатки и движение по каждому счёту", "Salden und Umsätze je Konto"),
    ("Transactions", "Операции", "Transaktionen"),
    ("Outlook", "Прогноз", "Ausblick"),
    ("Expected income for the next year", "Ожидаемый доход на следующий год", "Erwartete Erträge im nächsten Jahr"),
    ("Cash & deposits", "Денежные средства", "Liquidität"),
    ("Investments", "Инвестиции", "Anlagen"),
    ("Deposits", "Депозиты", "Einlagen"),
    ("Equities", "Акции", "Aktien"),
    ("Funds", "Фонды", "Fonds"),
    ("Metals", "Металлы", "Edelmetalle"),
    ("Bonds", "Облигации", "Anleihen"),
    ("Other", "Прочее", "Sonstige"),
    ("Test Bank · 3 accounts", "Тест Банк · 3 счёта", "Testbank · 3 Konten"),
    ("Broker 400PX0F", "Брокерский счёт 400PX0F", "Depot 400PX0F"),
    ("Capital AM", "УК «Капитал»", "Capital AM"),
    ("Unallocated metal accounts", "Обезличенные металлические счета", "Edelmetallkonten"),
    ("Government bonds", "Государственные облигации", "Staatsanleihen"),
    ("Structured notes", "Структурные продукты", "Strukturierte Produkte"),
    ("Portfolio", "Портфель", "Portfolio"),
    ("Equity index", "Индекс акций", "Aktienindex"),
    ("Bond index", "Индекс облигаций", "Anleihenindex"),
    ("Inflation", "Инфляция", "Inflation"),
    ("Value", "Стоимость", "Wert"),
    ("Result", "Результат", "Ergebnis"),
    ("Return", "Доходность", "Rendite"),
    ("Income", "Доход", "Ertrag"),
    ("Fees", "Комиссии", "Gebühren"),
    ("Month", "За месяц", "Monat"),
    ("Cumulative", "Нарастающим итогом", "Kumuliert"),
    ("Portfolio value", "Стоимость портфеля", "Portfoliowert"),
    ("Return · period", "Доходность за период", "Rendite · Zeitraum"),
    ("Holdings", "Активы", "Bestände"),
    ("Result by category", "Результат по категориям", "Ergebnis nach Kategorie"),
    ("Monthly return", "Доходность по месяцам", "Monatsrendite"),
    ("Return by category", "Доходность по категориям", "Rendite nach Kategorie"),
    ("Against benchmarks", "Сравнение с ориентирами", "Im Vergleich"),
    ("Income by month", "Доход по месяцам", "Ertrag nach Monat"),
    ("Structure", "Структура", "Struktur"),
    ("Portfolio *overview*", "Обзор *портфеля*", "Portfolio*übersicht*"),
    ("as of {{end}}", "на {{end}}", "zum {{end}}"),
    ("Returns were driven by **deposits**, while equities and metals lost value over the period.", "Доходность обеспечили **депозиты**, а акции и металлы за период подешевели.", "Die Rendite kam aus **Einlagen**; Aktien und Edelmetalle verloren im Zeitraum an Wert."),
    // the Quire theme's own widgets, named in the preview theme's language in the widgets library
    ("Toned card", "Тонированная карточка", "Getönte Karte"),
    ("Coloured by the result, with an arrow", "Цвет по результату, со стрелкой", "Nach Ergebnis gefärbt, mit Pfeil"),
    ("List", "Список", "Liste"),
    ("No header, light rows", "Без шапки, лёгкие строки", "Ohne Kopf, leichte Zeilen"),
    ("Columns", "Столбцы", "Säulen"),
    ("Line", "Линия", "Linie"),
    ("Area", "Область", "Fläche"),
    ("Small note", "Мелкая сноска", "Kleine Notiz"),
    ("Pages under section labels, with dotted leaders", "Страницы под названиями разделов, с отточием", "Seiten unter Abschnittsnamen, mit Punktlinien"),
];

/// A built-in widget the Quire theme names its own way, under the id the other built-in themes give the
/// same widget, so the widgets library lists it once.
pub fn same_widget<'a>(kind: &str, id: &'a str) -> &'a str {
    match (kind, id) {
        ("kpi", "big") => "hero",
        ("table", "standard") => "lines",
        ("bar", "bars") => "ranked",
        ("bar", "combo") => "monthly",
        ("bar", "range") => "bench",
        ("pie", "donut") => "ring",
        ("pie", "donut-table") => "top",
        ("waterfall", "waterfall") => "soft",
        ("treemap", "treemap") => "split",
        ("tiles", "tiles") => "horizons",
        ("heading", "eyebrow") => "cover",
        ("toc", "chapters") => "sections",
        _ => id,
    }
}

/// Widgets on a view the theme names another way take the theme's name for the same widget: "big" becomes
/// "hero" in a theme that has "hero" and not "big", and back. Returns how many changed.
pub fn same_views(doc: &mut Doc) -> usize {
    let mut n = 0;
    for w in doc.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()) {
        if w.variant.is_empty() {
            continue;
        }
        let Some(list) = doc.theme.styles.get(&w.kind) else { continue };
        if list.iter().any(|v| v.id == w.variant) {
            continue;
        }
        let shared = same_widget(&w.kind, &w.variant).to_string();
        let to = if list.iter().any(|v| v.id == shared) { Some(shared) } else { list.iter().find(|v| same_widget(&w.kind, &v.id) == w.variant).map(|v| v.id.clone()) };
        if let Some(to) = to.filter(|t| *t != w.variant) {
            w.variant = to;
            n += 1;
        }
    }
    n
}

/// A word of the built-in themes in a language (English when it has none).
pub fn tr(lang: &str, s: &str) -> String {
    let i = match lang.get(..2) {
        Some("ru") => 1,
        Some("de") => 2,
        _ => return s.to_string(),
    };
    WORDS.iter().find(|w| w.0 == s).map(|w| if i == 1 { w.1 } else { w.2 }).unwrap_or(s).to_string()
}

/// What a contents widget lists in the studio, which has no report pages of its own.
pub fn sample_toc(lang: &str) -> Vec<crate::vm::TocEntry> {
    let e = |kind: &'static str, text: &str, page: usize, desc: &str| crate::vm::TocEntry { kind, text: tr(lang, text), page, index: page.saturating_sub(1), description: if desc.is_empty() { String::new() } else { tr(lang, desc) } };
    vec![
        e("section", "Overview", 3, ""),
        e("page", "Portfolio overview", 3, "Assets, results and structure"),
        e("page", "Key figures", 5, "Returns by product and period"),
        e("section", "Detail", 7, ""),
        e("page", "Accounts", 7, "Balances and movements on each account"),
        e("page", "Transactions", 10, ""),
        e("section", "Outlook", 12, ""),
        e("page", "Outlook", 12, "Expected income for the next year"),
    ]
}

fn inline(id: &str, name: &str, columns: &[&str], rows: Vec<Vec<Json>>, date_field: Option<&str>) -> Dataset {
    Dataset {
        id: id.into(),
        name: name.into(),
        source: Source::Inline { columns: columns.iter().map(|s| s.to_string()).collect(), rows },
        date_field: date_field.map(String::from),
        ..Default::default()
    }
}

pub fn sample_data(lang: &str) -> Vec<Dataset> {
    let t = |s: &str| tr(lang, s);
    let months = ["2025-07-31", "2025-08-31", "2025-09-30", "2025-10-31", "2025-11-30", "2025-12-31", "2026-01-31", "2026-02-28", "2026-03-31", "2026-04-30", "2026-05-31", "2026-06-30"];
    let income: [f64; 12] = [4.2e6, 3.1e6, -1.8e6, 5.0e6, 2.2e6, 3.4e6, 7.4e6, 5.3e6, 6.2e6, -6.4e6, 6.8e6, 2.4e6];
    let ret = [0.012, 0.009, -0.006, 0.015, 0.007, 0.010, 0.021, 0.009, 0.003, -0.018, 0.007, 0.002];
    let mut value = 300e6;
    let mut cum = 0.0;
    let mut rows = Vec::new();
    for i in 0..12 {
        value += income[i];
        cum = (1.0 + cum) * (1.0 + ret[i]) - 1.0;
        rows.push(vec![json!(months[i]), json!(value), json!(income[i]), json!(ret[i]), json!(cum), json!(-(income[i].abs()) * 0.04)]);
    }
    let cats: Vec<Vec<Json>> = [
        ("Cash & deposits", "Deposits", 273_674_000.0, 29_314_000.0, 0.0385, "Test Bank · 3 accounts"),
        ("Investments", "Equities", 47_624_000.0, -3_864_000.0, -0.0742, "Broker 400PX0F"),
        ("Investments", "Funds", 18_669_000.0, -1_557_000.0, -0.077, "Capital AM"),
        ("Cash & deposits", "Metals", 7_381_000.0, -1_994_000.0, -0.2127, "Unallocated metal accounts"),
        ("Investments", "Bonds", 6_918_000.0, 980_000.0, 0.1417, "Government bonds"),
        ("Investments", "Other", 2_693_000.0, -180_000.0, -0.0638, "Structured notes"),
    ]
    .iter()
    .map(|(g, n, v, i, r, sub)| vec![json!(t(g)), json!(t(n)), json!(v), json!(i), json!(r), json!(t(sub))])
    .collect();
    let bench: Vec<Vec<Json>> = [("Portfolio", 0.0234), ("Equity index", -0.041), ("Bond index", 0.009), ("Inflation", 0.004), ("Deposits", 0.018)]
        .iter()
        .map(|(n, r)| vec![json!(t(n)), json!(r)])
        .collect();
    vec![
        inline("sm", "Sample · months", &["date", "value", "income", "ret", "cum", "fees"], rows, Some("date")),
        inline("sc", "Sample · categories", &["group", "name", "value", "income", "ret", "sub"], cats, None),
        inline("sb", "Sample · benchmarks", &["name", "ret"], bench, None),
    ]
}

fn m(id: &str, field: &str, agg: &str, label: &str, format: &str) -> Json {
    json!({"id": id, "field": field, "agg": agg, "label": label, "format": format})
}

/// The binding and content a design view is shown with in the studio.
/// The binding, content and title a design view is shown with, in the theme's language.
pub fn sample_widget(kind: &str, sample: &str, lang: &str) -> (Json, Json, String) {
    let (mut binding, mut content, title) = sample_widget_en(kind, sample);
    if let Some(ms) = binding.get_mut("measures").and_then(|m| m.as_array_mut()) {
        for m in ms {
            if let Some(l) = m.get("label").and_then(|l| l.as_str()).map(|l| tr(lang, l)) {
                m["label"] = json!(l);
            }
        }
    }
    if kind == "table" {
        // the category column reads as a heading, not a field name
        merge_json(&mut content, &json!({"table": {"cols": {"name": {"label": tr(lang, "Category")}, "__tree": {"label": tr(lang, "Category")}}}}));
    }
    if let Some(h) = content.pointer("/viz/highlight").and_then(|h| h.as_str()).map(|h| tr(lang, h)) {
        content["viz"]["highlight"] = json!(h);
    }
    (binding, content, tr(lang, &title))
}

fn sample_widget_en(kind: &str, sample: &str) -> (Json, Json, String) {
    let all = |ds: &str, dims: Vec<&str>, measures: Vec<Json>, extra: Json| {
        let mut b = json!({"dataset": ds, "dims": dims, "measures": measures, "window": "all", "filters": []});
        if let (Some(o), Some(e)) = (b.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                o.insert(k.clone(), v.clone());
            }
        }
        b
    };
    let cats = || all("sc", vec!["name"], vec![m("m0", "value", "sum", "Value", "currency")], json!({"sort": "value_desc"}));
    match (kind, sample) {
        ("kpi", "kpi-pct") => (all("sm", vec![], vec![m("m0", "cum", "closing", "", "percent")], json!({"window": "period"})), json!({}), "Return · period".into()),
        ("kpi", _) => (
            all("sm", vec![], vec![m("m0", "value", "closing", "", "currency"), m("m1", "income", "sum", "Income", "currency"), m("m2", "fees", "sum", "Fees", "currency")], json!({"window": "period"})),
            json!({}),
            "Portfolio value".into(),
        ),
        ("table", "grouped") => (
            all("sc", vec!["group", "name"], vec![m("m0", "value", "sum", "Value", "currency"), m("m1", "income", "sum", "Result", "currency"), m("m2", "ret", "max", "Return", "percent")], json!({"sort": "value_desc"})),
            json!({"table": {"cols": {"m1": {"signColor": true, "signed": true}, "m2": {"signColor": true, "signed": true}}}}),
            "Holdings".into(),
        ),
        ("table", _) => (
            all("sc", vec!["name"], vec![m("m0", "value", "sum", "Value", "currency"), m("m1", "income", "sum", "Result", "currency"), m("m2", "ret", "max", "Return", "percent")], json!({"sort": "value_desc"})),
            json!({"table": {"cols": {"m1": {"signColor": true, "signed": true}, "m2": {"signColor": true, "signed": true}}}}),
            "Result by category".into(),
        ),
        (_, "combo") => (
            all("sm", vec!["date:month"], vec![m("m0", "ret", "sum", "Month", "percent"), json!({"id": "m1", "field": "cum", "agg": "sum", "label": "Cumulative", "format": "percent", "chart": "line"})], json!({"window": "trailing12"})),
            // as in the templates: the running total on the right axis, labelled at its last value; months outside
            json!({"viz": {"seriesOpts": {"m0": {"labels": "all", "labelPos": "outside"}, "m1": {"axis": "right", "labels": "last"}}}}),
            "Monthly return".into(),
        ),
        (_, "ranked") => (all("sc", vec!["name"], vec![m("m0", "ret", "max", "Return", "percent")], json!({"sort": "value_desc"})), json!({}), "Return by category".into()),
        (_, "signed") => (all("sc", vec!["name"], vec![m("m0", "income", "sum", "Result", "currency"), m("m1", "ret", "max", "Return", "percent")], json!({})), json!({}), "Result by category".into()),
        (_, "bench") => (all("sb", vec!["name"], vec![m("m0", "ret", "sum", "", "percent")], json!({"sort": "data"})), json!({"viz": {"highlight": "Portfolio"}}), "Against benchmarks".into()),
        (_, "trend-value") => (all("sm", vec!["date:month"], vec![m("m0", "value", "closing", "Value", "currency")], json!({"window": "trailing12"})), json!({}), "Portfolio value".into()),
        (_, "trend") => (all("sm", vec!["date:month"], vec![m("m0", "income", "sum", "Income", "currency")], json!({"window": "ytd"})), json!({}), "Income by month".into()),
        _ => (cats(), json!({}), "Structure".into()),
    }
}

/// Size of a design view's preview on the 24 × 24 grid.
pub fn size(kind: &str) -> (u32, u32) {
    match kind {
        "kpi" => (8, 4),
        "table" => (12, 8),
        "pie" => (8, 7),
        "scale" | "tiles" => (12, 4),
        "heading" => (12, 3),
        "text" | "comment" => (12, 3),
        "divider" => (12, 1),
        "toc" => (12, 10),
        _ => (12, 7),
    }
}

/// A theme studio document for a theme.
pub fn studio_doc(theme: Theme, fonts: Vec<EmbeddedFont>) -> Doc {
    let mut doc = Doc::default();
    doc.kind = DocKind::Theme;
    doc.theme = with_base(theme);
    doc.meta.title = doc.theme.name.clone();
    doc.fonts = fonts;
    doc.page.cols = 24;
    doc.page.rows = 24;
    doc.page.gap = 6.0;
    doc.page.header = Band { show: false, ..Default::default() };
    doc.page.footer = Band { show: false, ..Default::default() };
    doc.params.grain = "custom".into();
    doc.params.start = Some(ymd(2026, 1, 1));
    doc.params.date = ymd(2026, 6, 30);
    doc.params.compare = "previous".into();
    doc.arrange = crate::layout::Options { auto: false, fill: false };
    rebuild(&mut doc);
    doc
}

/// Regenerates the studio pages from the theme's design views: one page per
/// view holding just its preview, so the studio can show each one whole.
pub fn rebuild(doc: &mut Doc) {
    let lang = doc.theme.locale.clone();
    // sample data follows the theme's language; replaced only when it changes
    let data = sample_data(&lang);
    if serde_json::to_value(&doc.datasets).ok() != serde_json::to_value(&data).ok() {
        doc.datasets = data;
    }
    let mut pages = Vec::new();
    for kind in KINDS {
        let Some(list) = doc.theme.styles.get(kind) else { continue };
        for sv in list {
            let (w, h) = size(kind);
            let (binding, content, title) = sample_widget(kind, &sv.sample, &lang);
            let mut opts = sv.opts.clone();
            if !opts.is_object() {
                opts = json!({});
            }
            // sample content sits under the design view's own settings
            let mut o = content;
            merge_json(&mut o, &opts);
            if kind == "table" {
                // previews never spill onto continuation pages
                merge_json(&mut o, &json!({"table": {"paginate": false}}));
            }
            match kind {
                "heading" => merge_json(&mut o, &json!({"text": opts.get("text").cloned().unwrap_or(json!(tr(&lang, "Portfolio *overview*"))), "subtitle": opts.get("subtitle").cloned().unwrap_or(json!(tr(&lang, "as of {{end}}")))})),
                "text" | "comment" => merge_json(&mut o, &json!({"text": opts.get("text").cloned().unwrap_or(json!(tr(&lang, "Returns were driven by **deposits**, while equities and metals lost value over the period.")))})),
                _ => {}
            }
            let needs_data = !matches!(kind, "heading" | "text" | "comment" | "divider" | "toc");
            let widget = Widget {
                id: format!("sv_{kind}_{}", sv.id),
                kind: kind.to_string(),
                x: 0,
                y: 0,
                w,
                h,
                title: if needs_data { title } else { String::new() },
                binding: if needs_data { serde_json::from_value(binding).ok() } else { None },
                opts: o,
                style: if sv.style.is_object() { sv.style.clone() } else { json!({}) },
                variant: sv.id.clone(),
                ..Default::default()
            };
            pages.push(Page { id: format!("studio_{kind}_{}", sv.id), title: sv.name.clone(), widgets: vec![widget], section: kind.into(), ..Default::default() });
        }
    }
    if pages.is_empty() {
        pages.push(Page { id: "studio_empty".into(), title: "Theme".into(), ..Default::default() });
    }
    doc.pages = pages;
}

/// Kinds whose preview height follows their content.
pub const FITTED: [&str; 4] = ["table", "text", "comment", "heading"];

/// Keys that are a widget's content, not its look: they stay out of design views.
const CONTENT_OPTS: [&str; 8] = ["text", "subtitle", "src", "author", "date", "chainKey", "studioLabel", "continues"];

/// Strips content from options so they can become a design view.
pub fn design_part(opts: &Json) -> Json {
    let mut o = if opts.is_object() { opts.clone() } else { json!({}) };
    if let Some(m) = o.as_object_mut() {
        for k in CONTENT_OPTS {
            m.remove(k);
        }
        if let Some(t) = m.get_mut("table").and_then(|t| t.as_object_mut()) {
            for k in ["cols", "order", "continues", "totalLabel"] {
                t.remove(k);
            }
        }
        if let Some(t) = m.get_mut("viz").and_then(|t| t.as_object_mut()) {
            for k in ["highlight", "note", "centerText", "deltaText"] {
                t.remove(k);
            }
        }
    }
    o
}

/// In the studio, an edit of a preview widget changes its design view.
/// Returns true when the patch was applied to the theme.
pub fn route_update(doc: &mut Doc, id: &str, patch: &Json) -> Result<bool, String> {
    if doc.kind != DocKind::Theme {
        return Ok(false);
    }
    let Some((pi, wi)) = doc.find_widget(id) else { return Ok(false) };
    let w = &doc.pages[pi].widgets[wi];
    if w.variant.is_empty() {
        return Ok(true); // labels and other studio furniture aren't editable
    }
    let (kind, vid) = (w.kind.clone(), w.variant.clone());
    let sv = doc.theme.styles.get_mut(&kind).and_then(|l| l.iter_mut().find(|s| s.id == vid)).ok_or("That design view no longer exists.")?;
    if let Some(o) = patch.get("opts") {
        let mut o = o.clone();
        if let Some(m) = o.as_object_mut() {
            if matches!(kind.as_str(), "heading" | "text" | "comment") {
                // the sample text isn't part of the look
                m.remove("text");
                m.remove("subtitle");
            }
            if let Some(t) = m.get_mut("table").and_then(|t| t.as_object_mut()) {
                t.remove("cols");
            }
        }
        if !sv.opts.is_object() {
            sv.opts = json!({});
        }
        merge_json(&mut sv.opts, &o);
    }
    if let Some(s) = patch.get("style") {
        if !sv.style.is_object() {
            sv.style = json!({});
        }
        merge_json(&mut sv.style, s);
    }
    if let Some(n) = patch.get("variant_name").and_then(|v| v.as_str()) {
        sv.name = n.trim().to_string();
    }
    if let Some(n) = patch.get("variant_description").and_then(|v| v.as_str()) {
        sv.description = n.trim().to_string();
    }
    rebuild(doc);
    Ok(true)
}

fn unique_id(list: &[StyleVariant], base: &str) -> String {
    let base: String = base.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let base = if base.trim_matches('-').is_empty() { "view".to_string() } else { base.trim_matches('-').to_string() };
    let mut id = base.clone();
    let mut n = 2;
    while list.iter().any(|s| s.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Adds a design view: a copy of `from` (in the theme) or the given look. Returns its id.
pub fn add_variant(theme: &mut Theme, kind: &str, name: &str, from: Option<&str>, opts: Option<Json>, style: Option<Json>) -> Result<String, String> {
    if !KINDS.contains(&kind) {
        return Err(format!("Themes don't style '{kind}' widgets."));
    }
    let list = theme.styles.entry(kind.to_string()).or_default();
    let mut sv = match from.and_then(|f| list.iter().find(|s| s.id == f)) {
        Some(s) => s.clone(),
        None => StyleVariant { sample: base_styles().get(kind).and_then(|l| l.first()).map(|s| s.sample.clone()).unwrap_or_default(), opts: json!({}), style: json!({}), ..Default::default() },
    };
    if let Some(o) = opts {
        sv.opts = design_part(&o);
    }
    if let Some(s) = style {
        sv.style = s;
    }
    sv.name = if name.trim().is_empty() { format!("{} copy", sv.name) } else { name.trim().to_string() };
    sv.id = unique_id(list, &sv.name);
    let id = sv.id.clone();
    list.push(sv);
    Ok(id)
}

pub fn remove_variant(theme: &mut Theme, kind: &str, id: &str) {
    if let Some(l) = theme.styles.get_mut(kind) {
        l.retain(|s| s.id != id);
    }
}

/// Moves a design view; the first of a kind is the default for new widgets.
pub fn move_variant(theme: &mut Theme, kind: &str, id: &str, to: usize) {
    if let Some(l) = theme.styles.get_mut(kind) {
        if let Some(i) = l.iter().position(|s| s.id == id) {
            let s = l.remove(i);
            l.insert(to.min(l.len()), s);
        }
    }
}

/// Puts a theme on a template: colours, fonts and design views. Widgets keep
/// their design view ids, so a theme with the same ids restyles them.
pub fn apply(doc: &mut Doc, theme: &Theme, fonts: &[EmbeddedFont]) {
    let locale = doc.theme.locale.clone();
    let currency = doc.theme.currency.clone();
    // the widget types belong to the document, not the theme: they stay
    let types = std::mem::take(&mut doc.theme.widget_types);
    doc.theme = theme.clone();
    doc.theme.widget_types = types;
    paint_bands(doc);
    same_views(doc);
    let renamed = doc.theme.upgrade();
    for w in doc.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()) {
        crate::model::upgrade_scale(w, &renamed);
    }
    if doc.theme.locale.is_empty() {
        doc.theme.locale = locale;
    }
    if doc.theme.currency.is_empty() {
        doc.theme.currency = currency;
    }
    for f in fonts {
        if !doc.fonts.iter().any(|x| x.family == f.family && x.file == f.file) {
            doc.fonts.push(f.clone());
        }
    }
}

/// Every design view's (and the restyle's kept view changes') own typeface for these text
/// roles is dropped, so the views follow the role again.
pub fn unpin_families(theme: &mut Theme, roles: &[String]) {
    let drop = |style: &mut Json| {
        for r in roles {
            if let Some(f) = style.pointer_mut(&format!("/fonts/{r}")).and_then(|f| f.as_object_mut()) { f.remove("family"); }
        }
    };
    for v in theme.styles.values_mut().flat_map(|l| l.iter_mut()) { drop(&mut v.style); }
    if let Some(vs) = theme.look.get_mut("views").and_then(|v| v.as_object_mut()) {
        for p in vs.values_mut() { if p["style"].is_object() { drop(&mut p["style"]); } }
    }
}

/// The text roles a theme edit gives a typeface: those it names, and with a new base font every
/// role without a typeface of its own. Their design views follow (a typeface a view kept would
/// otherwise go on showing whatever the role says).
pub fn families_set(theme: &Theme, patch: &Json) -> Vec<String> {
    let mut roles: Vec<String> = patch["fonts"].as_object().into_iter().flatten()
        .filter(|(_, f)| f.get("family").is_some()).map(|(r, _)| r.clone()).collect();
    if patch.get("font_family").is_some() {
        let all: std::collections::BTreeSet<String> = theme.styles.values().flatten()
            .filter_map(|v| v.style["fonts"].as_object()).flat_map(|m| m.keys().cloned()).chain(theme.fonts.keys().cloned()).collect();
        roles.extend(all.into_iter().filter(|r| theme.fonts.get(r).and_then(|f| f.family.as_ref()).is_none()));
    }
    roles
}

/// After a restyle: a typeface the views keep for a role belongs to the role. The one most of
/// them use becomes the role's (unless it already is, or is the base font it inherits), and
/// the views drop it, so the Typography settings change them all.
pub fn families_to_roles(theme: &mut Theme) {
    let mut by_role: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for v in theme.styles.values().flatten() {
        for (r, f) in v.style["fonts"].as_object().into_iter().flatten() {
            if let Some(fam) = f["family"].as_str() { *by_role.entry(r.clone()).or_default().entry(fam.to_string()).or_default() += 1; }
        }
    }
    for (r, fams) in by_role {
        let Some((fam, _)) = fams.iter().max_by_key(|(_, n)| **n) else { continue };
        let own = theme.fonts.get(&r).and_then(|f| f.family.clone());
        let inherited = own.clone().unwrap_or_else(|| theme.font_family.clone());
        if *fam != inherited {
            theme.fonts.entry(r.clone()).or_default().family = Some(fam.clone());
        }
        // views keeping another typeface than the role's stay as they are
        let role_fam = fam.clone();
        for v in theme.styles.values_mut().flat_map(|l| l.iter_mut()) {
            if v.style.pointer(&format!("/fonts/{r}/family")).and_then(|x| x.as_str()) == Some(role_fam.as_str()) {
                if let Some(f) = v.style.pointer_mut(&format!("/fonts/{r}")).and_then(|f| f.as_object_mut()) { f.remove("family"); }
            }
        }
        if let Some(vs) = theme.look.get_mut("views").and_then(|v| v.as_object_mut()) {
            for p in vs.values_mut() {
                if p.pointer(&format!("/style/fonts/{r}/family")).and_then(|x| x.as_str()) == Some(role_fam.as_str()) {
                    if let Some(f) = p.pointer_mut(&format!("/style/fonts/{r}")).and_then(|f| f.as_object_mut()) { f.remove("family"); }
                }
            }
        }
    }
}

const BAND_KEYS: [&str; 4] = ["fill", "line", "line_color", "tabs"];

fn band_key<'a>(b: &'a mut crate::model::Band, k: &str) -> Option<&'a mut String> {
    match k {
        "fill" => Some(&mut b.fill),
        "line" => Some(&mut b.line),
        "line_color" => Some(&mut b.line_color),
        "tabs" => Some(&mut b.tabs),
        _ => None,
    }
}

/// The header and footer as the theme has them (`look.page`, from a restyle): the bands' own
/// look comes back first (whatever an earlier theme painted goes), then this theme paints its
/// own, keeping what was there.
pub fn paint_bands(doc: &mut Doc) {
    let own = std::mem::take(&mut doc.page.own_bands);
    let page = &mut doc.page;
    for (band, b) in [("header", &mut page.header), ("footer", &mut page.footer)] {
        for (k, v) in own[band].as_object().into_iter().flatten() {
            if let (Some(slot), Some(v)) = (band_key(b, k), v.as_str()) { *slot = v.into(); }
        }
    }
    let paint = doc.theme.look["page"].clone();
    let mut kept = json!({});
    for (band, b) in [("header", &mut page.header), ("footer", &mut page.footer)] {
        for k in BAND_KEYS {
            let Some(v) = paint[band][k].as_str() else { continue };
            let slot = band_key(b, k).unwrap();
            kept[band][k] = json!(slot.clone());
            *slot = v.into();
        }
    }
    if kept.as_object().is_some_and(|m| !m.is_empty()) { page.own_bands = kept; }
}

/// A band set by hand is the template's own from then on: no theme takes it back.
pub fn bands_set_by_hand(page: &mut crate::model::PageSetup, patch: &Json) {
    for band in ["header", "footer"] {
        for k in BAND_KEYS {
            if !patch[band][k].is_null() {
                if let Some(m) = page.own_bands.get_mut(band).and_then(|b| b.as_object_mut()) { m.remove(k); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quire_names_shared_widgets_once_and_templates_follow() {
        let q = builtin("quire").unwrap();
        for (kind, old, shared) in [("kpi", "big", "hero"), ("table", "standard", "lines"), ("pie", "donut", "ring")] {
            let l = &q.styles[kind];
            assert!(l.iter().any(|v| v.id == shared) && !l.iter().any(|v| v.id == old), "{kind}: {:?}", l.iter().map(|v| &v.id).collect::<Vec<_>>());
        }
        // a template on the plain views takes the shared ones in Quire
        let mut d = crate::sample::template();
        let w0 = d.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()).find(|w| w.kind == "kpi").unwrap();
        w0.variant = "big".into();
        let id = w0.id.clone();
        apply(&mut d, &q, &[]);
        let w = d.pages.iter().flat_map(|p| &p.widgets).find(|w| w.id == id).unwrap();
        assert_eq!(w.variant, "hero");
        // and back on a theme that only has the plain name
        let plain = builtin("plain").unwrap();
        apply(&mut d, &plain, &[]);
        let w = d.pages.iter().flat_map(|p| &p.widgets).find(|w| w.id == id).unwrap();
        assert_eq!(w.variant, "big", "the plain theme names it “big”");
    }

    use super::*;

    #[test]
    fn view_typefaces_follow_the_text_roles() {
        let mut t = builtin("plain").unwrap();
        t.font_family = "Arial".into();
        t.fonts.entry("kpi".into()).or_default().family = None;
        let kpi_views: Vec<String> = t.styles["kpi"].iter().map(|v| v.id.clone()).collect();
        for v in t.styles.get_mut("kpi").unwrap() { v.style["fonts"]["kpi"]["family"] = json!("Arial Black"); }
        t.look["views"] = json!({"kpi/x": {"style": {"fonts": {"kpi": {"family": "Arial Black", "size": 9}}}}});
        // a restyle's typeface in every KPI view becomes the role's
        families_to_roles(&mut t);
        assert_eq!(t.fonts["kpi"].family.as_deref(), Some("Arial Black"));
        assert!(t.styles["kpi"].iter().all(|v| v.style.pointer("/fonts/kpi/family").is_none()), "{kpi_views:?}");
        assert!(t.look.pointer("/views/kpi~1x/style/fonts/kpi/family").is_none());
        // a view kept on a typeface by hand lets go when the role is set
        t.styles.get_mut("kpi").unwrap()[0].style["fonts"]["kpi"]["family"] = json!("Spectral");
        let roles = families_set(&t, &json!({"fonts": {"kpi": {"family": "Rubik"}}}));
        assert_eq!(roles, vec!["kpi".to_string()]);
        unpin_families(&mut t, &roles);
        assert!(t.styles["kpi"][0].style.pointer("/fonts/kpi/family").is_none());
        // a new base font reaches the roles that inherit it
        assert!(families_set(&t, &json!({"font_family": "Rubik"})).contains(&"body".to_string()) || t.fonts.get("body").and_then(|f| f.family.as_ref()).is_some());
    }

    #[test]
    fn a_theme_takes_its_header_and_footer_away_with_it() {
        let mut doc = Doc::default();
        doc.page.header.fill = "".into();
        doc.page.footer.line = "top".into();
        let mut painted = builtin("plain").unwrap();
        painted.look["page"] = json!({"header": {"fill": "#ffffff", "tabs": "left"}, "footer": {"fill": "#ffd800", "line": "none"}});
        apply(&mut doc, &painted, &[]);
        assert_eq!((doc.page.header.fill.as_str(), doc.page.header.tabs.as_str()), ("#ffffff", "left"));
        assert_eq!((doc.page.footer.fill.as_str(), doc.page.footer.line.as_str()), ("#ffd800", "none"));
        // the saved file keeps what to go back to
        let back: Doc = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        let mut doc = back;
        // a band coloured by hand stays the template's own
        bands_set_by_hand(&mut doc.page, &json!({"footer": {"fill": "#000000"}}));
        doc.page.footer.fill = "#000000".into();
        apply(&mut doc, &builtin("plain").unwrap(), &[]);
        assert_eq!((doc.page.header.fill.as_str(), doc.page.header.tabs.as_str()), ("", "none"));
        assert_eq!((doc.page.footer.fill.as_str(), doc.page.footer.line.as_str()), ("#000000", "top"));
        assert!(doc.page.own_bands.is_null());
    }

    #[test]
    fn studio_shows_every_view_and_edits_reach_the_theme() {
        let mut doc = studio_doc(builtin("plain").unwrap(), vec![]);
        let views: usize = doc.theme.styles.values().map(|l| l.len()).sum();
        let shown = doc.pages.iter().flat_map(|p| &p.widgets).filter(|w| !w.variant.is_empty()).count();
        assert_eq!(views, shown);
        // editing a preview edits its design view
        let id = "sv_kpi_card".to_string();
        assert!(route_update(&mut doc, &id, &json!({"opts": {"viz": {"arrow": true}}, "style": {"background": "#eeeeee"}})).unwrap());
        let sv = doc.theme.variant("kpi", "card").unwrap();
        assert_eq!(sv.opts["viz"]["arrow"], json!(true));
        assert_eq!(sv.style["background"], json!("#eeeeee"));
        // a template widget using the view renders with it, its own settings winning
        let w = Widget { kind: "kpi".into(), variant: "card".into(), style: json!({"background": "#ffffff"}), ..Default::default() };
        let e = effective(&doc.theme, &w);
        assert_eq!(e.opts["viz"]["arrow"], json!(true));
        assert_eq!(e.style["background"], json!("#ffffff"));
        // new views get unique ids
        let a = add_variant(&mut doc.theme, "kpi", "Card", Some("card"), None, None).unwrap();
        assert_ne!(a, "card");
    }
}
