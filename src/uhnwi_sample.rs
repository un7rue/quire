//! "Modern UHNWI" private-banking report: the 18-page Test Private Banking
//! design built from the Modern UHNWI theme. Almost every widget picks a
//! design view from the theme (`"variant"`) and carries only its content.

use crate::uhnwi_sql;
use crate::model::*;
use crate::period::ymd;
pub use crate::uhnwi_theme::theme;
use crate::uhnwi_theme::*;
use serde_json::{json, Value as Json};

fn w(v: Json) -> Widget {
    serde_json::from_value(v).expect("sample widget")
}

fn m(id: &str, field: &str, agg: &str, label: &str, format: &str) -> Json {
    json!({"id": id, "field": field, "agg": agg, "label": label, "format": format})
}

fn with(mut a: Json, extra: Json) -> Json {
    merge_json(&mut a, &extra);
    a
}

fn bind(ds: &str, dims: &[&str], measures: Vec<Json>, extra: Json) -> Json {
    with(json!({"dataset": ds, "dims": dims, "measures": measures, "window": "all", "filters": [], "sort": "data"}), extra)
}

fn eq(field: &str, value: &str) -> Json {
    json!({"field": field, "op": "=", "value": value})
}

trait Tap {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}
impl Tap for Widget {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

/// A widget using one of the theme's design views.
fn view(kind: &str, variant: &str, rect: (u32, u32, u32, u32), extra: Json) -> Widget {
    w(with(json!({"kind": kind, "variant": variant, "x": rect.0, "y": rect.1, "w": rect.2, "h": rect.3}), extra))
}

fn kpi(variant: &str, title: &str, rect: (u32, u32, u32, u32), binding: Json, viz: Json) -> Widget {
    view("kpi", variant, rect, json!({"title": title, "binding": binding, "opts": {"viz": viz}}))
}

fn heading(variant: &str, rect: (u32, u32, u32, u32), opts: Json) -> Widget {
    view("heading", variant, rect, json!({"opts": opts}))
}

fn section(text: &str, rect: (u32, u32, u32, u32), meta: &str) -> Widget {
    heading("section", rect, json!({"text": text, "meta": meta}))
}

/// A fact card: label, a value from the data, a note under it (a KPI card).
fn fact(label: &str, rect: (u32, u32, u32, u32), binding: Json, viz: Json, size: f64) -> Widget {
    kpi("card", label, rect, binding, viz).tap(|w| w.style = json!({"fonts": {"kpi": {"size": size}}}))
}

fn text(variant: &str, rect: (u32, u32, u32, u32), body: &str, opts: Json) -> Widget {
    view("text", variant, rect, json!({"opts": with(json!({"text": body}), opts)}))
}

fn ink_rule(x: u32, y: u32, wd: u32) -> Widget {
    view("divider", "ink", (x, y, wd, 1), json!({}))
}

fn vrule(x: u32, y: u32, h: u32) -> Widget {
    view("divider", "vertical", (x, y, 1, h), json!({}))
}

fn table(variant: &str, title: &str, rect: (u32, u32, u32, u32), binding: Json, opts: Json) -> Widget {
    view("table", variant, rect, json!({"title": title, "binding": binding, "opts": opts}))
}

// ------------------------------------------------------------------ template

/// The demo bank's test client: the person this report is about.
pub const TEST_CLIENT: &str = "Кирилл Шарихин";


/// The report's data sources: (id, name, date field). Each is a query on the
/// demo bank database's own tables (see uhnwi_sql.rs).
const SOURCES: [(&str, &str, Option<&str>); 42] = [
    ("clients", "Клиенты", None),
    ("port", "Портфель · итоги", None),
    ("cat", "Продукты", None),
    ("bridge", "Как изменился капитал", None),
    ("fcsum", "Поступления на 12 месяцев · по видам", None),
    ("cls", "Классы активов", None),
    ("cur", "Валюты", None),
    ("top5", "Крупнейшие позиции", None),
    ("mon", "Результаты по месяцам", Some("date")),
    ("am", "Доходность по месяцам", Some("date")),
    ("hz", "Горизонты", None),
    ("acct", "Счета · итоги", None),
    ("cmp", "Ориентиры · шкала", None),
    ("acls", "Классы · результат", None),
    ("inst", "Инструменты · лидеры", None),
    ("contrib", "Вклад счетов", None),
    ("vs", "Сравнения", None),
    ("bench", "Ориентиры", None),
    ("cohort", "Когорта", None),
    ("accounts", "Счета клиента", None),
    ("hold", "Состав брокерского счёта", None),
    ("mix", "Структура и результат", None),
    ("duh", "Состав ДУ", None),
    ("dumix", "Состав ДУ · классы", None),
    ("about", "О портфеле", None),
    ("bank", "Банковские счета", None),
    ("cfm", "Поступления по месяцам", None),
    ("fcm", "Прогноз по месяцам", None),
    ("cfi", "Поступления по инструментам", None),
    ("inflow", "Приток за 12 месяцев", None),
    ("srcs", "Источники дохода", None),
    ("principal", "Возврат капитала", None),
    ("flows", "Сделки и операции · итоги", None),
    ("bigtrades", "Крупнейшие сделки", None),
    ("conv", "Валютные конверсии", None),
    ("ops", "Операции", None),
    ("deals", "Сделки по инструментам", None),
    ("cashops", "Вводы и выводы", None),
    ("charges", "Налоги и комиссии", None),
    ("policy", "Полис", None),
    ("polhist", "Полис по месяцам", None),
    ("products", "Продукты клиента", None),
];

/// Reads every data source from the demo bank database's tables, for the
/// client chosen in the toolbar and the report period.
fn from_database(doc: &mut Doc) {
    doc.connections = vec![Connection {
        id: "demo".into(),
        name: "Demo bank".into(),
        kind: "duckdb".into(),
        path: crate::dbconn::demo_path().unwrap_or_else(|| "data/demo.duckdb".into()),
        url: String::new(),
    }];
    let db = |query: String| Source::Db { connection: "demo".into(), query };
    for (id, name, date_field) in SOURCES {
        let query = if id == "clients" { uhnwi_sql::CLIENTS.into() } else { uhnwi_sql::query(id).unwrap_or_default().trim().into() };
        doc.datasets.push(Dataset { id: id.into(), name: name.into(), source: db(query), date_field: date_field.map(Into::into), ..Default::default() });
    }
    // every client, for the toolbar's client picker
    doc.datasets.push(Dataset { id: "client_list".into(), name: "Список клиентов".into(), source: db(uhnwi_sql::CLIENT_LIST.into()), ..Default::default() });
}

pub fn template() -> Doc {
    let mut doc = template_raw();
    crate::uhnwi_theme::fold_views(&mut doc);
    // no dividers between halves: the gap tells them apart (halves of 12 and 12)
    doc.remove_split_dividers();
    // monthly values and their running total: two axes, the total labelled at its last value
    doc.split_series_axes();
    doc
}

/// The template as written, its widgets on the looks they were designed with.
pub fn template_raw() -> Doc {
    let mut doc = Doc::default();
    doc.meta.title = "Отчёт по активам и доходности".into();
    // published in the Reports area as "uhnwi"
    doc.meta.report_id = "uhnwi".into();
    doc.meta.subtitle = "Персональный отчёт".into();
    doc.meta.organization = "Test Private Banking".into();
    doc.meta.description = "Modern UHNWI: портфель, результаты, ориентиры, счета, прогноз денежного потока, сделки.".into();
    doc.theme = theme();

    doc.page.cols = 24;
    doc.page.rows = 24;
    doc.page.gap = 6.0;
    doc.page.margin = 30.0;
    doc.page.header = Band {
        show: true,
        height: 30.0,
        tabs: "left".into(),
        left: Slot { font: FontSpec { size: Some(6.4), weight: Some(600), color: Some(BODY.into()), ..Default::default() }, ..Default::default() },
        ..Default::default()
    };
    doc.page.footer = Band {
        show: true,
        height: 22.0,
        line: "top".into(),
        line_color: LINE.into(),
        left: Slot { text: "{{data:Клиенты.banker}} · {{data:Клиенты.email}} · {{data:Клиенты.phone}}".into(), ..Default::default() },
        center: Slot { text: "Период {{start}} — {{end}} · сформирован {{today}}".into(), ..Default::default() },
        right: Slot { text: "{{page}}".into(), ..Default::default() },
        ..Default::default()
    };
    // the custom range: from 1 January of this year to today
    let today = chrono::Local::now().date_naive();
    doc.params.grain = "custom".into();
    doc.params.start = Some(ymd(chrono::Datelike::year(&today), 1, 1));
    doc.params.date = today;
    doc.params.compare = "none".into();
    // the client is picked from the database's clients; the products (source systems)
    // from the client's own, shown by name — every query reads them where it reads the accounts
    doc.param_defs = vec![
        ParamDef {
            name: "client".into(),
            label: "Клиент".into(),
            default: TEST_CLIENT.into(),
            source: Some(ParamSource { dataset: "client_list".into(), field: "client".into(), label: String::new() }),
            ..Default::default()
        },
        ParamDef {
            name: "product".into(),
            label: "Продукты".into(),
            kind: "list".into(),
            multi: true,
            default: "All".into(),
            source: Some(ParamSource { dataset: "products".into(), field: "src_cd".into(), label: "product".into() }),
            // who the client is doesn't depend on the products shown
            columns: [("clients".to_string(), String::new()), ("client_list".to_string(), String::new())].into_iter().collect(),
            ..Default::default()
        },
    ];
    doc.params.values.insert("client".into(), TEST_CLIENT.into());
    doc.params.values.insert("product".into(), "All".into());

    from_database(&mut doc);

    // (title, section, group, nav label, description, widgets)
    let pages: Vec<(&str, &str, &str, &str, &str, Vec<Widget>)> = vec![
        ("Обложка", "", "", "", "", cover()),
        ("Содержание", "Обзор портфеля", "", "", "", contents()),
        ("Портфель — кратко", "Обзор портфеля", "", "", "Активы, результат и доходность за период", summary()),
        ("Обзор портфеля", "Обзор портфеля", "", "", "Структура, классы активов, валюты", overview()),
        ("Результаты", "Ключевые показатели", "", "", "Доходность по классам и валютам", results()),
        ("Сравнение и ориентиры", "Ключевые показатели", "", "", "Бенчмарки и историческая динамика", benchmarks()),
        ("{{row.title}}", "Детальная информация", "BROK", "Обзор", "Обзор и доходность счёта{{row.closed_note}}", broker_overview()),
        ("{{row.title}} · состав", "Детальная информация", "BROK", "Состав", "Позиции счёта", broker_holdings(0)),
        ("{{row.title}} · состав (продолжение)", "Детальная информация", "BROK", "", "", broker_holdings(1)),
        ("Доверительное управление · {{row.strategy}}", "Детальная информация", "DU", "Обзор", "{{row.manager}} · {{row.acct_numb}}{{row.closed_note}}", du_overview()),
        ("Доверительное управление · состав", "Детальная информация", "DU", "Состав", "Фонды в составе мандата", du_holdings()),
        ("{{row.title}}", "Детальная информация", "INS", "Обзор", "Страхование жизни · до {{row.policy_end}}{{row.closed_note}}", ins_overview()),
        ("Банковские счета", "Детальная информация", "BANK", "Обзор", "Тест Банк · обзор и структура", bank_overview()),
        ("Банковские счета · структура", "Детальная информация", "BANK", "Структура", "Вклады, счета и металлы", bank_structure()),
        ("Факт и прогноз на 12 месяцев", "Прогноз", "", "", "Получено + помесячный прогноз", monthly_receipts()),
        ("График поступлений по месяцам", "Прогноз", "", "", "По инструментам · помесячно", monthly_detail()),
        ("Сделки и операции", "Детальная информация", "", "", "Обзор активности за период", trades_overview()),
        ("Сделки по инструментам", "Детальная информация", "", "", "Крупнейшие сделки периода", trades_detail()),
        ("Движение средств, налоги и комиссии", "Детальная информация", "", "", "Вводы, выводы, НДФЛ и комиссии", money_detail()),
        ("Юридическая информация", "Юридическая", "", "", "Оговорки и раскрытие рисков", legal()),
    ];
    doc.pages.clear();
    for (title, section, group, label, description, widgets) in pages {
        let id = doc.new_id("p");
        let plain = section.is_empty();
        let widgets = widgets.into_iter().map(|x| doc.instantiate(x)).collect();
        doc.pages.push(Page {
            id,
            title: title.into(),
            widgets,
            section: section.into(),
            plain,
            group: group.into(),
            label: label.into(),
            description: description.into(),
            frame: if plain { "$frame".into() } else { String::new() },
            // the bank portfolio shows when the client has bank accounts
            show_if: if group == "BANK" { "{{data:port.has_bank}}".into() } else { String::new() },
            // continuation pages stay out of the contents
            toc_hidden: title.contains("(продолжение)"),
            // each brokerage (IIS too), DU and insurance account gets its own pages
            repeat: match group {
                "BROK" | "DU" | "INS" => Repeat { dataset: "accounts".into(), field: "kind".into(), values: vec![group.into()], label: "acct_numb".into() },
                _ => Repeat::default(),
            },
            ..Default::default()
        });
    }
    // continuations and linked widgets point at the instantiated tables ("@key" → its id)
    let keys: std::collections::HashMap<String, String> = doc.pages.iter().flat_map(|p| &p.widgets)
        .filter_map(|w| w.opt_str("chainKey").filter(|k| !k.is_empty()).map(|k| (format!("@{k}"), w.id.clone()))).collect();
    for p in &mut doc.pages {
        for w in &mut p.widgets {
            if let Some(id) = w.opts.pointer("/table/continues").and_then(|v| v.as_str()).and_then(|k| keys.get(k)) {
                w.opts["table"]["continues"] = json!(id);
            }
            if let Some(id) = w.opts.get("follows").and_then(|v| v.as_str()).and_then(|k| keys.get(k)) {
                w.opts["follows"] = json!(id);
            }
            if let Some(o) = w.opts.as_object_mut() {
                o.remove("chainKey");
            }
        }
    }
    doc.arrange = crate::layout::Options { auto: false, fill: false };
    doc.normalize();
    doc
}

// ------------------------------------------------------------------ data


// ------------------------------------------------------------------ pages

fn base64_svg(svg: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(svg.as_bytes())
}

fn cover() -> Vec<Widget> {
    let logo = "data:image/svg+xml;base64,".to_string() + &base64_svg(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40"><rect width="40" height="40" rx="9" fill="#a12a2a"/><text x="20" y="27.5" font-family="Manrope" font-size="19" font-weight="800" fill="#f5f0e6" text-anchor="middle">T</text></svg>"##
    ));
    vec![
        w(json!({"kind": "image", "x": 1, "y": 0, "w": 1, "h": 2, "opts": {"src": logo}})),
        text("body", (2, 0, 9, 1), "**Test Private Banking**", json!({})).tap(|w| w.style = json!({"fonts": {"body": {"size": 9.5, "color": INK}}})),
        text("small", (2, 1, 9, 1), "Частное банковское обслуживание", json!({})),
        text("small", (17, 0, 6, 1), "КОНФИДЕНЦИАЛЬНО", json!({"align": "right"})).tap(|w| w.style = json!({"fonts": {"body": {"size": 6.4, "weight": 600, "tracking": 0.2}}})),
        heading("cover", (1, 5, 14, 8), json!({"text": "Отчёт по активам\nи *доходности*"})),
        view("divider", "hair", (1, 13, 2, 1), json!({"style": {"color": ACCENT, "width": 0.9}})),
        text("body", (1, 14, 12, 1), "за период с {{start}} по {{end}}", json!({})).tap(|w| w.style = json!({"fonts": {"body": {"family": "@display", "size": 11, "weight": 400, "color": BODY}}})),
        view("divider", "hair", (1, 19, 22, 1), json!({"style": {"color": INK, "width": 0.75}})),
        heading("person", (1, 20, 10, 3), json!({"text": "{{data:Клиенты.banker}}", "subtitle": "{{data:Клиенты.email}} · {{data:Клиенты.phone}}"})),
        heading("person", (16, 20, 7, 3), json!({"eyebrow": "Дата формирования", "text": "{{today}}", "align": "right"})).tap(|w| w.style = json!({"fonts": {"eyebrow": {"color": SOFT}}})),
    ]
}

fn contents() -> Vec<Widget> {
    vec![
        heading("page", (0, 0, 24, 3), json!({"text": "Содержание", "meta": "{{pages}} стр. · на {{end}}"})),
        view("toc", "sections", (0, 3, 24, 21), json!({"title": "", "opts": {"toc": {"icons": {
            "Обзор портфеля": "pie", "Ключевые показатели": "chart", "Детальная информация": "safe", "Прогноз": "trend-up", "Юридическая": "doc"}}}})),
    ]
}

fn icon_map() -> Json {
    json!({"Депозиты": "percent", "Брокерские": "trend-up", "Паевые": "layers", "ПИФ": "layers", "Металлы": "diamond", "Доверительное": "briefcase",
        "Денежные средства": "cash", "Денежные средства и металлы": "cash", "Акции": "bars", "Фонды": "layers", "Драгметаллы": "diamond", "Прочие валюты": "globe", "Прочие": "grid", "Облигации": "doc",
        "Страхование": "shield", "Полис": "shield", "Накопительный": "piggy-bank", "Вклад «": "percent", "Платёжный": "wallet",
        "Российский рубль": "flag-ru", "Рубль": "flag-ru", "Доллар": "flag-us", "Юань": "flag-cn",
        // currency pairs: the currency bought or sold against roubles
        "USD /": "flag-us", "CNY /": "flag-cn", "EUR /": "flag-eu", "GBP /": "flag-gb", "CHF /": "flag-ch", "HKD /": "flag-hk", "JPY /": "flag-jp"})
}

/// A product column: an icon chip per product.
fn product_cols(name_key: &str) -> Json {
    json!({name_key: {"label": "Продукт", "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}, "bold": true, "width": 40}})
}

/// Trend chips after the names of the best and the worst rows, ranked by `by`.
fn trend_marks(by: &str) -> Json {
    json!({"on": true, "by": by, "show": "icon",
        "best": {"n": 1, "icon": "trend-up", "color": GAIN, "iconFill": GAIN_TINT},
        "worst": {"n": 1, "icon": "trend-down", "color": LOSS, "iconFill": LOSS_TINT}})
}

fn summary() -> Vec<Widget> {
    let p = |measures: Vec<Json>| bind("port", &[], measures, json!({}));
    // the headline and the three figures that matter most, side by side above the line
    let card = |title: &str, x: u32, wd: u32, icon: &str, measures: Vec<Json>, viz: Json| kpi("icon", title, (x, 0, wd, 4), p(measures), viz).tap(|w| w.style = json!({"titleIcon": {"icon": icon}}));
    let mut v = vec![
        kpi("hero", "Итого активы на {{end}}, ₽", (0, 0, 8, 4), p(vec![m("m0", "value", "sum", "", "number")]),
            json!({"note": "период {{start}} — {{end}}", "noteColor": "ink"})).tap(|w| w.style = json!({"fonts": {"kpi": {"size": 25}}})),
        card("Результат в деньгах", 8, 5, "cash", vec![m("m0", "income", "sum", "", "currency"), m("m1", "dep_income", "sum", "Доход от вкладов", "currency"), m("m2", "inv_income", "sum", "Доход от инвестиций", "currency")], json!({"compact": "on"})),
        card("Доходность за период", 13, 5, "trend-up", vec![m("m0", "ret", "sum", "", "percent"), m("m1", "vs_peers", "sum", "vs похожие клиенты", "number"), m("m2", "vs_index", "sum", "vs Индекс МосБиржи", "number")], json!({"decimals": 2})),
        card("Регулярный доход · 12 мес", 18, 6, "calendar", vec![m("m0", "fc", "sum", "", "currency"), m("m1", "fc_month", "sum", "Среднемесячно", "currency"), m("m2", "horizon", "sum", "Горизонт, мес.", "number")], json!({"compact": "on"})),
        ink_rule(0, 4, 24),
    ];
    // under "Результат в деньгах": the products, then how the capital changed
    let mut cols = product_cols("product");
    merge_json(&mut cols, &json!({"product": {"width": 43}, "m0": {"bar": {"on": true, "only": true, "color": SHARE}, "width": 6}, "m1": {"bold": true}, "m2": {"signColor": true, "signed": true, "bold": true, "total": "sum"},
        "m3": {"signColor": true, "signed": true, "decimals": 2, "total": "none"}}));
    v.push(table("lines", "Результат по продуктам", (0, 5, 12, 12),
        bind("cat", &["product"], vec![with(m("m0", "value", "sum", "Доля", "percent"), json!({"calc": "share"})), m("m1", "value", "sum", "Стоимость ₽", "number"), m("m2", "income", "sum", "Результат", "currency"), m("m3", "ret", "max", "Доходн.", "percent")], json!({})),
        json!({"table": {"totalLabel": "Итого портфель", "row_h": 22, "fill": true, "cols": cols, "extremes": trend_marks("m3")}}))
        .tap(|w| { w.style = json!({"fonts": {"table_cell": {"size": 8.4}, "table_head": {"size": 6}}}); w.subtitle = "по видам активов: ОМС и фонды отдельно, где бы ни хранились · доходность — результат к вложенному".into(); }));
    v.push(view("waterfall", "soft", (0, 17, 12, 7), json!({"title": "Как изменился капитал", "subtitle": "млн ₽ · пополнения, выводы и доход вкладов и инвестиций",
        "binding": bind("bridge", &["step"], vec![m("m0", "amount", "sum", "", "number")], json!({"sort": "data"})),
        "opts": {"viz": {"bridge": true, "units": "millions", "decimals": 1, "totalLabel": "Конец", "legend": "none", "extremes": false, "callout": "last"}}})));
    v.push(vrule(12, 5, 19));
    // under "Доходность за период": the return in plain answers, then against the benchmarks
    v.push(section("Доходность в цифрах", (13, 5, 11, 1), ""));
    let fig = |label: &str, rect: (u32, u32, u32, u32), field: &str, note: &str, signed: bool| {
        let f = if field == "pos_months" { "" } else { "percent" };
        let agg = if field == "pos_months" { "text" } else { "max" };
        fact(label, rect, bind("port", &[], vec![m("m0", field, agg, "", f)], json!({})),
            json!({"decimals": 1, "note": note, "noteColor": "ink", "valueColor": if signed { "sign" } else { "font" }, "signed": signed}), 14.0)
    };
    v.extend([
        fig("В годовом выражении", (13, 6, 4, 3), "ann_ret", "при том же темпе", true),
        fig("Лучший месяц", (17, 6, 4, 3), "best_m_ret", "{{data:port.best_m}}", true),
        fig("Слабейший месяц", (21, 6, 3, 3), "worst_m_ret", "{{data:port.worst_m}}", true),
    ]);
    v.push(view("bar", "ranked", (13, 9, 11, 8), json!({"title": "Доходность против ориентиров", "subtitle": "за период",
        "binding": bind("bench", &["name"], vec![m("m0", "ret", "sum", "", "percent")], json!({"sort": "value_desc",
            "filters": [{"field": "name", "op": "in", "value": "Ваш портфель,Похожие клиенты · медиана,Индекс МосБиржи,Вклад по ключевой ставке,Инфляция"}]})),
        "opts": {"viz": {"highlight": "Ваш портфель", "decimals": 1}}})).tap(|w| w.style = json!({"pad": 0})));
    // under "Прогноз денежного потока": the next twelve months' payments by kind
    v.push(table("lines", "Поступления на 12 месяцев", (13, 17, 11, 7), bind("fcsum", &["kind"], vec![m("m0", "n", "sum", "Выплат", "number"), m("m1", "next", "text", "Ближайшая", ""), m("m2", "amount", "sum", "Сумма, ₽", "number")], json!({"sort": "data"})),
        json!({"totals": true, "table": {"totalLabel": "Итого за 12 месяцев", "row_h": 18, "cols": {
            "kind": {"label": "Вид", "bold": true, "width": 40, "icon": {"map": {"Проценты": "percent", "Дивиденды": "coins", "Купоны": "doc", "Возврат": "rotate-ccw"}, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}},
            "m0": {"color": SOFT, "decimals": 0, "zero": "dash"}, "m1": {"color": SOFT, "align": "right", "total": "none"}, "m2": {"bold": true, "decimals": 0, "zero": "dash"}}}}))
        .tap(|w| { w.style = json!({"pad": 0}); w.subtitle = "до НДФЛ".into(); }));
    v
}

fn overview() -> Vec<Widget> {
    let mut v = vec![
        kpi("hero", "Итого активы на {{end}}, ₽", (0, 0, 10, 4), bind("port", &[], vec![m("m0", "value", "sum", "", "number")], json!({})), json!({})),
        fact("Профиль риска", (10, 0, 4, 3), bind("clients", &[], vec![m("m0", "profile", "text", "", "")], json!({})), json!({}), 10.0),
        fact("Крупнейший класс", (14, 0, 5, 3), bind("cls", &["class"], vec![m("m0", "value", "sum", "", "currency")], json!({})), json!({"kpiGroup": "name", "shareAside": true}), 10.0),
        fact("Крупнейшая валюта", (19, 0, 5, 3), bind("cur", &["currency"], vec![m("m0", "value", "sum", "", "currency")], json!({})), json!({"kpiGroup": "name", "shareAside": true}), 10.0),
        ink_rule(0, 4, 24),
    ];
    let mut cols = product_cols("product");
    merge_json(&mut cols, &json!({"product": {"sub": "m3", "width": 54}, "m0": {"bar": {"on": true, "only": true, "color": SHARE}, "width": 7}, "m1": {"bold": true}, "m2": {"color": SOFT, "decimals": 0, "total": "none"}, "m3": {"hidden": true}}));
    v.push(table("lines", "Баланс активов по продуктам", (0, 5, 11, 9),
        bind("cat", &["product"], vec![with(m("m0", "value", "sum", "Доля", "percent"), json!({"calc": "share"})), m("m1", "value", "sum", "Сумма, ₽", "number"), with(m("m2", "value", "sum", " ", "percent"), json!({"calc": "share"})), m("m3", "held", "text", " ", "")], json!({})),
        json!({"table": {"totalLabel": "Итого активы", "row_h": 15, "head_h": 16, "fill": true, "cols": cols}})));
    v.push(table("lines", "Крупнейшие позиции · топ-5", (0, 14, 11, 10),
        bind("top5", &["name"], vec![with(m("m0", "value", "sum", "#", "number"), json!({"calc": "rank"})), m("m1", "value", "sum", "Сумма, ₽", "currency"), m("m2", "share", "sum", "Доля", "percent"), m("m3", "sub", "text", " ", "")], json!({})),
        json!({"table": {"totalLabel": "Итого топ-5", "row_h": 19, "head_h": 16, "order": ["m0", "name", "m1", "m2"], "cols": {
            "m0": {"color": SOFT, "width": 6, "total": "none"},
            "name": {"label": "Позиция", "sub": "m3", "bold": true, "icon": {"map": {"Накопительный": "piggy-bank", "Вклад": "percent", "Платёжный": "wallet", "ПИФ": "layers", "Фонд": "layers"}, "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}},
            "m3": {"hidden": true}, "m1": {"bold": true}, "m2": {"color": SOFT, "decimals": 0}}}})));
    v.push(vrule(11, 5, 19));
    v.push(view("treemap", "split", (12, 5, 12, 8), json!({"title": "Структура по классам активов", "subtitle": "площадь ↔ доля класса",
        "binding": bind("cls", &["class"], vec![m("m0", "value", "sum", "", "currency")], json!({"sort": "value_desc"})),
        "opts": {"viz": {"icons": icon_map(), "iconsAuto": true, "palette": ["$class1", "$class2", "$class3", "$class4", "$class5", "$class6"]}}})));
    v.push(view("pie", "top", (12, 14, 12, 10), json!({"title": "Структура по валютам",
        "binding": bind("cur", &["currency"], vec![m("m0", "value", "sum", "", "currency")], json!({"sort": "value_desc"})),
        "opts": {"viz": {"icons": icon_map(), "iconsAuto": true, "centerLabel": "рубль"}}})));
    v
}

fn results() -> Vec<Widget> {
    let card = |title: &str, x: u32, b: Json| kpi("card-result", title, (x, 0, 4, 3), b, json!({}));
    let mut v = vec![
        kpi("hero-signed", "Чистый результат с начала года", (0, 0, 8, 4), bind("port", &[], vec![m("m0", "income", "sum", "", "currency"), m("m1", "ret", "sum", "", "percent")], json!({})),
            json!({"note": "Доходность {1} · период {{start}} — {{end}}", "noteColor": "sign", "noteDecimals": 2})),
        card("Доход от депозитов", 8, bind("cat", &[], vec![m("m0", "income", "sum", "", "currency")], json!({"filters": [eq("product", "Депозиты и вклады")]}))),
        card("{{data:port.inv_title}}", 12, bind("cat", &[], vec![m("m0", "income", "sum", "", "currency")], json!({"filters": [{"field": "product", "op": "!=", "value": "Депозиты и вклады"}]}))),
        card("Лучший месяц · {{data:port.best_inc_month}}", 16, bind("mon", &[], vec![m("m0", "income", "max", "", "currency")], json!({}))),
        card("Слабейший месяц · {{data:port.worst_inc_month}}", 20, bind("mon", &[], vec![m("m0", "income", "min", "", "currency")], json!({}))),
        ink_rule(0, 4, 24),
    ];
    let mut cols = product_cols("product");
    merge_json(&mut cols, &json!({"product": {"label": "Категория"}, "m0": {"bar": {"on": true, "only": true, "color": GAIN, "negColor": LOSS}, "width": 30},
        "m1": {"signColor": true, "signed": true, "bold": true}, "m2": {"signColor": true, "signed": true, "decimals": 2}}));
    v.push(table("lines", "Результат по категориям", (0, 5, 12, 10),
        bind("cat", &["product"], vec![m("m0", "income", "sum", "Вклад в результат", "number"), m("m1", "income", "sum", "Результат", "currency"), m("m2", "ret", "max", "Доходн.", "percent")], json!({})),
        json!({"totals": false, "table": {"row_h": 20, "cols": cols, "extremes": trend_marks("m2")}})).tap(|w| w.subtitle = "по видам активов: ОМС и фонды отдельно, где бы ни хранились · доходность — результат к вложенному".into()));
    v.push(view("waterfall", "soft", (0, 15, 12, 9), json!({"title": "Доход по месяцам и накопленный итог",
        "binding": bind("mon", &["date:month"], vec![m("m0", "income", "sum", "Доход", "currency")], json!({}))})));
    v.push(vrule(12, 5, 19));
    v.push(view("tiles", "results", (13, 5, 11, 8), json!({"title": "Доходность по классам активов",
        "binding": bind("cls", &["class"], vec![m("m0", "income", "sum", "", "currency"), m("m1", "ret", "max", "", "percent")], json!({})),
        "opts": {"viz": {"icons": icon_map(), "iconsAuto": true, "cols": 3}}})));
    v.push(view("tiles", "results", (13, 13, 11, 5), json!({"title": "Доходность по валютам",
        "binding": bind("cur", &["currency"], vec![m("m0", "income", "sum", "", "currency"), m("m1", "ret", "max", "", "percent")], json!({"filters": [{"field": "currency", "op": "!=", "value": "Прочие валюты"}]})),
        "opts": {"viz": {"icons": icon_map(), "iconsAuto": true, "cols": 3, "markExtremes": false}}})));
    v.push(section("Лучшие и худшие инструменты", (13, 18, 11, 1), ""));
    v.extend(leaders("ALL", (13, 19, 11, 5), ("Лидеры роста", "Аутсайдеры")));
    v
}

/// Best and worst: two short ranked tables with a trend chip in the title.
fn leaders(acct: &str, rect: (u32, u32, u32, u32), labels: (&str, &str)) -> Vec<Widget> {
    let half = rect.2 / 2;
    let tbl = |title: &str, x: u32, wd: u32, sort: &str, up: bool| {
        // leaders gained, outsiders lost: an outsider list shows only what lost money
        let sign = if up { json!({"field": "income", "op": ">", "value": "0"}) } else { json!({"field": "income", "op": "<", "value": "0"}) };
        let t = table("leaders", title, (x, rect.1, wd, rect.3),
            bind("inst", &["name"], vec![with(m("m2", "income", "sum", "#", "number"), json!({"calc": "rank"})), m("m0", "income", "sum", "Результат", "currency"), m("m1", "ret", "max", "%", "percent")], json!({"sort": sort, "limit": 3, "filters": [eq("acct", acct), sign]})),
            json!({"table": {"order": ["m2", "name", "m0"], "row_h": 24, "cols": {
                // the rank stays available (Columns) but is off: the names get the room
                "m2": {"hidden": true, "width": 10, "align": "center", "pill": {"on": true, "fill": if up { GAIN_TINT } else { LOSS_TINT }, "color": if up { GAIN } else { LOSS }}},
                "name": {"label": "Инструмент", "width": 54, "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "m0": {"signColor": true, "signed": true, "bold": true, "sub": "m1"}, "m1": {"hidden": true, "signed": true, "decimals": 1}}},
                "emptyText": if up { "Нет позиций с доходом за период" } else { "Убыточных позиций за период нет" }}));
        // no inset: the whole list fits its place
        // three rows in larger type: read at a glance
        let big = json!({"table_cell": {"size": 8.8}});
        if up {
            t.tap(|w| w.style = json!({"pad": 0, "fonts": big}))
        } else {
            t.tap(|w| w.style = json!({"pad": 0, "titleIcon": {"icon": "trend-down", "fill": LOSS_TINT, "color": LOSS}, "fonts": with(big.clone(), json!({"title": {"color": LOSS}})), "rule": "left", "ruleColor": LINE2}))
        }
    };
    vec![tbl(labels.0, rect.0, half, "value_desc", true), tbl(labels.1, rect.0 + half, rect.2 - half, "value_asc", false)]
}

fn contribution_donuts(x: u32, y: u32, wd: u32, h: u32) -> Vec<Widget> {
    let d = |month: &str, title: &str, xx: u32, color: &str, base: &str| {
        view("pie", "contribution", (xx, y, wd / 2, h), json!({"title": title,
            "binding": bind("contrib", &["account"], vec![m("m0", "pp", "sum", "", "number")], json!({"filters": [eq("month", month)]})),
            "opts": {"viz": {"baseColor": base, "suffix": " п.п.", "decimals": 2, "labelSuffix": ""}}, "style": {"fonts": {"title": {"color": color}}}}))
    };
    // the period's best and worst months, named from the data
    let right = d("worst", "{{data:port.worst_month}} · слабейший месяц", x + wd / 2, LOSS, "$lossMid").tap(|w| {
        w.style["rule"] = json!("left");
        w.style["ruleColor"] = json!(LINE2);
    });
    vec![d("best", "{{data:port.best_month}} · лучший месяц", x, GAIN, "$gainMid"), right]
}

fn benchmarks() -> Vec<Widget> {
    let vs = |title: &str, x: u32| kpi("card-result", title, (x, 0, 4, 3), bind("vs", &[], vec![m("m0", "pp", "sum", "", "number")], json!({"filters": [eq("metric", title)]})), json!({"decimals": 1, "suffix": " п.п."}));
    let mut v = vec![
        kpi("hero-signed", "Доходность с начала года · TWR", (0, 0, 8, 4), bind("am", &[], vec![m("m0", "cum", "closing", "", "percent")], json!({"filters": [eq("acct", "ALL")]})),
            json!({"decimals": 2, "note": "**{{data:port.quartile}}** среди похожих клиентов · {{data:port.pctile}}-й перцентиль", "noteBoldColor": GAIN})),
        vs("VS похожие клиенты", 8),
        vs("VS индекс МосБиржи", 12),
        vs("VS депозит", 16),
        vs("VS инфляция", 20),
        ink_rule(0, 4, 24),
        view("tiles", "horizons", (0, 5, 12, 3), json!({"binding": bind("hz", &["label"], vec![m("m0", "ret", "sum", "", "percent")], json!({"filters": [eq("acct", "ALL")]}))})),
        view("bar", "monthly-titled", (0, 8, 12, 9), json!({"title": "Доходность за месяц и итог",
            "binding": bind("am", &["date:month"], vec![m("m0", "ret", "sum", "рост", "percent"), with(m("m1", "cum", "sum", "итог", "percent"), json!({"chart": "line", "color": INK}))], json!({"filters": [eq("acct", "ALL")]}))})),
        section("Вклад счетов в результат месяца, п.п.", (0, 17, 12, 1), ""),
        vrule(12, 5, 19),
        view("bar", "ranked", (13, 5, 11, 8), json!({"title": "Портфель против ориентиров", "subtitle": "доходность за период · ранжирование",
            "binding": bind("bench", &["name"], vec![m("m0", "ret", "sum", "", "percent")], json!({"sort": "value_desc",
                "filters": [{"field": "name", "op": "!=", "value": "Похожие клиенты · медиана"}, {"field": "name", "op": "!=", "value": "Вклад по ключевой ставке"}]})),
            "opts": {"viz": {"highlight": "Ваш портфель"}}})),
        section("Клиенты, похожие на вас", (13, 13, 11, 1), "{{data:port.peers}} · тот же профиль"),
        view("bar", "cohort", (13, 14, 11, 4), json!({"title": "Позиция по доходности", "subtitle": "{{data:port.ahead_txt}}",
            "binding": bind("cmp", &["name"], vec![m("m0", "ret", "sum", "", "percent")], json!({"filters": [eq("acct", "COHORT")]})), "opts": {"viz": {"highlight": "Вы"}}})),
    ];
    let metrics = ["Доходность YTD", "Устойчивость · просадка", "Прибыльных месяцев", "Коэффициент Шарпа", "Волатильность", "Валют в портфеле"];
    for (i, metric) in metrics.iter().enumerate() {
        let pct = matches!(*metric, "Доходность YTD" | "Устойчивость · просадка" | "Волатильность");
        let f = if pct { "percent" } else { "number" };
        let (x, wd) = match i % 3 {
            0 => (13, 4),
            1 => (17, 4),
            _ => (21, 3),
        };
        v.push(kpi("stat", metric, (x, 18 + (i as u32 / 3) * 3, wd, 3), bind("cohort", &[], vec![m("m0", "value", "sum", "", f), m("m1", "median", "sum", "", f), m("m2", "diff", "sum", "", f)], json!({"filters": [eq("metric", metric)]})),
            json!({"decimals": if pct { 1 } else if *metric == "Коэффициент Шарпа" { 2 } else { 0 }, "noteDecimals": if pct { 1 } else if *metric == "Коэффициент Шарпа" { 2 } else { 0 }, "valueColor": if pct { "sign" } else { "ink" }, "signed": pct})));
    }
    v.extend(contribution_donuts(0, 18, 12, 6));
    v
}

// ---- account pages

/// On a page repeated per account: the account it shows.
const ROW_ACCT: &str = "{{row.acct_cd}}";

fn account_header(badge: &str, eyebrow: &str, title: &str, acct: &str, cards: Vec<(&str, &str, &str, &str)>, value_label: &str) -> Vec<Widget> {
    let mut v = vec![heading("account", (0, 0, 8, 4), json!({"badge": badge, "eyebrow": eyebrow, "text": title}))];
    let mut x = 8;
    let wd = if cards.len() == 3 { 4 } else { 5 };
    for (t, field, format, note) in &cards {
        let fmt = if *format == "pp" { "number" } else { format };
        v.push(kpi("card-result", t, (x, 0, wd, 4), bind("acct", &[], vec![m("m0", field, "sum", "", fmt)], json!({"filters": [eq("acct", acct)]})),
            json!({"decimals": if *format == "percent" { 2 } else if *format == "pp" { 1 } else { 0 }, "suffix": if *format == "pp" { " п.п." } else { "" }, "note": note})));
        x += wd;
    }
    v.push(kpi("value-right", value_label, (x, 0, 24 - x, 4), bind("acct", &[], vec![m("m0", "value", "sum", "", "currency")], json!({"filters": [eq("acct", acct)]})), json!({})));
    v.push(ink_rule(0, 4, 24));
    v
}

fn change_cards(acct: &str, y: u32, names: [&str; 4], ops: [&str; 2], inv: [&str; 2]) -> Vec<Widget> {
    let b = |measures: Vec<Json>| bind("acct", &[], measures, json!({"filters": [eq("acct", acct)]}));
    vec![
        section("Как изменились ваши активы за период", (0, y, 24, 1), "изменение стоимости = ваши операции + инвестиционный результат"),
        kpi("card", names[0], (0, y + 1, 6, 4), b(vec![m("m0", "value", "sum", "", "currency"), m("m1", "value_chg", "sum", "", "percent")]), json!({"note": "на {{end}} · {1} к началу", "noteColor": "ink"}))
            .tap(|w| w.style = json!({"fonts": {"kpi": {"size": 18}}})),
        kpi("breakdown", names[1], (6, y + 1, 6, 4), b(vec![m("m0", "net", "sum", "", "currency"), m("m1", "deposits", "sum", ops[0], "currency"), m("m2", "withdrawals", "sum", ops[1], "currency")]), json!({})),
        kpi("breakdown", names[2], (12, y + 1, 6, 4), b(vec![m("m0", "inv", "sum", "", "currency"), m("m1", "divs", "sum", inv[0], "currency"), m("m2", "reval", "sum", inv[1], "currency")]), json!({})),
        kpi("card-result", names[3], (18, y + 1, 6, 4), b(vec![m("m0", "change", "sum", "", "currency"), m("m1", "start", "sum", "", "currency")]), json!({"note": "начало периода · {1}", "noteColor": "ink"}))
            .tap(|w| w.style = json!({"border": ACCENT_LINE, "borderWidth": 1.1, "fonts": {"kpi": {"size": 18}}})),
    ]
}

fn account_body(acct: &str, hl: &str) -> Vec<Widget> {
    vec![
        section("Доходность", (0, 10, 11, 1), ""),
        view("tiles", "horizons", (0, 11, 11, 3), json!({"binding": bind("hz", &["label"], vec![m("m0", "ret", "sum", "", "percent")], json!({"filters": [eq("acct", acct)]}))})),
        view("bar", "monthly", (0, 14, 11, 6), json!({"title": "За месяц и нарастающим итогом",
            "binding": bind("am", &["date:month"], vec![m("m0", "ret", "sum", "за месяц", "percent"), with(m("m1", "cum", "sum", "нарастающим итогом", "percent"), json!({"chart": "line", "color": INK}))], json!({"filters": [eq("acct", acct)]}))})),
        view("bar", "bench", (0, 20, 11, 4), json!({"title": "Сравнение с ориентирами",
            "binding": bind("cmp", &["name"], vec![m("m0", "ret", "sum", "", "percent")], json!({"filters": [eq("acct", acct)]})), "opts": {"viz": {"highlight": hl}}})),
        vrule(11, 10, 14),
    ]
}

fn class_results(acct: &str, rect: (u32, u32, u32, u32), title: &str, sub: &str) -> Widget {
    // a ranked bar chart: result by class, its return and value underneath
    view("bar", "classes", rect, json!({"title": title, "subtitle": sub,
        "binding": bind("acls", &["class"], vec![m("m0", "income", "sum", "Результат", "currency"), m("m1", "ret", "max", "Доходность", "percent"), m("m2", "value", "sum", "Стоимость", "number")], json!({"filters": [eq("acct", acct)]}))}))
}

fn broker_overview() -> Vec<Widget> {
    let mut v = account_header("{{row.badge}}", "{{row.type_name}}{{row.closed_note}}", "Обзор и доходность", ROW_ACCT, vec![
        ("Доходность · период", "twr", "percent", "TWR · нарастающим итогом"),
        ("Результат за период", "result", "currency", "переоценка + доход"),
        ("Против ориентиров", "vs", "pp", "к индексу МосБиржи"),
    ], "Стоимость счёта");
    v.extend(change_cards(ROW_ACCT, 5, ["Стоимость счёта сейчас", "Чистые операции", "Инвестиционный результат", "Изменение стоимости"], ["Пополнения", "Выводы"], ["Дивиденды и купоны", "Рыночная переоценка"]));
    v.extend(account_body(ROW_ACCT, "Счёт"));
    v.push(class_results(ROW_ACCT, (12, 10, 12, 7), "Результат по классам активов", "вклад в результат счёта за период · доходность — к вложенному, не TWR"));
    v.push(section("Лучшие и худшие инструменты", (12, 17, 12, 1), ""));
    v.extend(leaders(ROW_ACCT, (12, 18, 12, 6), ("Лидеры роста", "Аутсайдеры")));
    v
}

fn holdings_table(rect: (u32, u32, u32, u32), piece: u32) -> Widget {
    let mut t = json!({"totalLabel": "Итого по счёту · {n} позиций", "paginate": true,
        "cols": {
            "__tree": {"label": "Инструмент", "sub": "m9", "width": 31, "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}},
            "m9": {"hidden": true},
            // a bond maturing soon: its row tinted, with the days left after its name
            "m10": {"hidden": true, "rules": [{"op": "between", "value": "0", "value2": "30", "row": true, "fill": LOSS_TINT, "accent": LOSS, "tag": "{v} дней до погашения", "tagFill": LOSS, "tagColor": PAPER}]},
            "m0": {"zero": "dash", "groupHide": true, "decimals": 0, "total": "none"},
            "m1": {"decimals": 2, "groupHide": true, "zero": "dash", "total": "none"},
            "m2": {"decimals": 2, "groupHide": true, "zero": "dash", "total": "none"},
            "m3": {"signColor": true, "signed": true, "decimals": 1, "groupHide": true, "total": "none"},
            "m4": {"bold": true},
            "m5": {"color": "$dividend", "zero": "dash"},
            "m6": {"signColor": true, "signed": true, "bold": true},
            // a position's return is its result over what went in; the account's own is the TWR (the card above), so no total
            "m7": {"signColor": true, "signed": true, "decimals": 1, "total": "none"},
            "m8": {"color": SOFT, "decimals": 1}
        },
        // the best holding and the two worst, by return: tinted rows with a trend icon
        "extremes": {"on": true, "by": "m7", "show": "both",
            "best": {"n": 1, "icon": "trend-up", "color": GAIN, "fill": GAIN_TINT},
            "worst": {"n": 2, "icon": "trend-down", "color": LOSS, "fill": LOSS_TINT}}});
    if piece == 1 {
        t["continues"] = json!("@holdings");
    }
    table("grouped", "", rect,
        bind("hold", &["bucket", "name"], vec![
            m("m0", "qty", "max", "Кол-во", "number"),
            m("m1", "avg", "max", "Ср. цена ·\nFIFO", "number"),
            m("m2", "price", "max", "Тек. цена", "number"),
            m("m3", "chg", "max", "Изм. цены", "percent"),
            m("m4", "value", "sum", "Стоимость, ₽", "number"),
            m("m5", "divs", "sum", "Получено, ₽", "number"),
            m("m6", "income", "sum", "Доход, ₽", "number"),
            with(m("m7", "income", "sum", "Доходность", "percent"), json!({"divide_by": "start"})),
            m("m8", "share", "sum", "Доля", "percent"),
            m("m9", "isin", "text", " ", ""),
            m("m10", "days_left", "max", "Дней до погашения", "number"),
        ], json!({"filters": [eq("acct", ROW_ACCT)]})),
        json!({"chainKey": if piece == 0 { "holdings" } else { "" }, "table": t}))
}

/// "Total for the account": the result and value on one line, then the allocation coloured by result.
/// Widgets kept with the end of a table (its "chainKey"): while the table continues, they go after its last rows.
fn after_table(key: &str, mut v: Vec<Widget>) -> Vec<Widget> {
    for w in &mut v {
        w.opts["follows"] = json!(format!("@{key}"));
    }
    v
}

fn total_bar(acct: &str, label: &str, y: u32) -> Vec<Widget> {
    // the account's result and value: a repeated account's come with its row, the bank's from its totals
    let q = |col: &str| if acct == ROW_ACCT { format!("{{{{row.{col}}}}}") } else { format!("{{{{sql: select {col} from счета_итоги where acct = '{acct}'}}}}") };
    vec![
        ink_rule(0, y, 24),
        heading("section", (0, y + 1, 24, 1), json!({"text": label, // the result smaller, in its sign's colour; the value large and bold
            "meta": format!("{{${}-|результат {}}}\u{2003}\u{2003}{{+|**{}**}}", q("result_tone"), q("result_txt"), q("value_txt"))}))
            .tap(|w| w.style = json!({"fonts": {"heading": {"size": 8.4, "upper": false, "tracking": 0}, "caption": {"size": 10, "weight": 500, "color": INK}}})),
        view("bar", "share-result", (0, y + 2, 24, 22 - y), json!({"binding": bind("mix", &["kind"], vec![m("m0", "value", "sum", "", "currency"), m("m1", "result", "sum", "", "currency")], json!({"filters": [eq("acct", acct)]}))})),
    ]
}

fn broker_holdings(part: u32) -> Vec<Widget> {
    let mut v = vec![heading("account", (0, 0, 7, 4), json!({"badge": "{{row.badge}}", "eyebrow": "{{row.type_name}}{{row.closed_note}}", "text": "Состав портфеля"}))];
    let acct = || bind("acct", &[], vec![m("m0", "value", "sum", "", "currency")], json!({"filters": [eq("acct", ROW_ACCT)]}));
    if part == 0 {
        v.push(fact("Структура", (7, 0, 4, 4), bind("acls", &["short"], vec![m("m0", "value", "sum", "", "number")], json!({"filters": [eq("acct", ROW_ACCT)]})), json!({"kpiGroup": "shares", "note": "{names}"}), 13.0));
        // the best and worst positions by return, named under the value
        let pos = |sort: &str| bind("hold", &["name"], vec![m("m0", "ret", "max", "", "percent")], json!({"sort": sort, "filters": [eq("acct", ROW_ACCT)]}));
        v.push(kpi("card-result", "Лучшая позиция", (11, 0, 4, 4), pos("value_desc"), json!({"decimals": 1, "kpiGroup": "value", "note": "{name}"})));
        v.push(kpi("card-result", "Худшая позиция", (15, 0, 4, 4), pos("value_asc"), json!({"decimals": 1, "kpiGroup": "value", "note": "{name}"})));
        v.push(kpi("value-right", "Стоимость счёта", (19, 0, 5, 4), acct(), json!({})));
        v.push(ink_rule(0, 4, 24));
        v.push(holdings_table((0, 5, 24, 19), 0));
    } else {
        v.push(kpi("card-result", "Деньги и металлы", (7, 0, 4, 4), bind("acls", &[], vec![m("m0", "ret", "max", "", "percent")], json!({"filters": [eq("acct", ROW_ACCT), eq("class", "Денежные средства и металлы")]})), json!({"decimals": 1, "note": "{{row.cash_note}}"})));
        v.push(kpi("card-result", "Облигации", (11, 0, 4, 4), bind("acls", &[], vec![m("m0", "ret", "max", "", "percent")], json!({"filters": [eq("acct", ROW_ACCT), eq("class", "Облигации")]})), json!({"decimals": 1, "note": "{{row.bond_note}}"})));
        v.push(kpi("card-result", "Итог по счёту", (15, 0, 4, 4), bind("acct", &[], vec![m("m0", "twr", "sum", "", "percent")], json!({"filters": [eq("acct", ROW_ACCT)]})), json!({"decimals": 1, "note": "полная доходность"})));
        v.push(kpi("value-right", "Стоимость счёта", (19, 0, 5, 4), acct(), json!({})));
        v.push(ink_rule(0, 4, 24));
        v.push(holdings_table((0, 5, 24, 12), 1));
        let note = text("small", (12, 17, 12, 1), "Доход — переоценка цен плюс дивиденды и купоны   ·   Полная доходность — к стоимости счёта", json!({"align": "right"}));
        v.extend(after_table("holdings", std::iter::once(note).chain(total_bar(ROW_ACCT, "Итого по счёту", 18)).collect()));
    }
    v
}

fn du_overview() -> Vec<Widget> {
    let mut v = account_header("{{row.badge}}", "{{row.type_name}}{{row.closed_note}}", "Обзор и доходность", ROW_ACCT, vec![
        ("Доходность · период", "twr", "percent", "TWR · нарастающим итогом"),
        ("Результат за период", "result", "currency", "{{row.acct_name}}"),
    ], "Стоимость портфеля");
    v.extend(change_cards(ROW_ACCT, 5, ["Стоимость портфеля сейчас", "Чистые операции", "Инвестиционный результат", "Изменение стоимости"], ["Пополнения", "Выводы"], ["Рыночная переоценка фондов", "Дивиденды · в составе паёв"]));
    v.extend(account_body(ROW_ACCT, "Портфель"));
    v.push(table("list", "Состав по классам активов", (12, 10, 12, 8),
        bind("dumix", &["class"], vec![m("m0", "share", "sum", "Доля · полоса", "percent"), m("m1", "value", "sum", "Стоимость", "currency"), m("m2", "share", "sum", "Доля портфеля", "percent"), m("m3", "detail", "text", "Состав класса", "")], json!({"filters": [eq("acct", ROW_ACCT)]})),
        json!({"table": {"row_h": 30, "cols": {"class": {"bold": true, "sub": "m3", "width": 32}, "m0": {"bar": {"on": true, "only": true, "color": SHARE}, "width": 36}, "m1": {"bold": true, "sub": "m2"}, "m2": {"hidden": true, "decimals": 1, "suffix": " портфеля"}, "m3": {"hidden": true}}}}))
        .tap(|w| w.subtitle = "структура мандата на {{end}}".into()));
    v.push(table("card", "О портфеле", (12, 18, 12, 6), bind("about", &["key"], vec![m("m0", "value", "text", " ", "")], json!({"filters": [eq("acct", ROW_ACCT)]})),
        json!({"table": {"layout": "facts", "factCols": 2, "factSize": 1.2, "hideHead": true, "row_h": 17}}))
        .tap(|w| w.style = json!({"fonts": {"title": {"color": SOFT}, "table_cell": {"size": 7.6}}})));
    v
}

fn du_holdings() -> Vec<Widget> {
    let mut v = vec![heading("account", (0, 0, 9, 4), json!({"badge": "{{row.badge}}", "eyebrow": "{{row.type_name}}{{row.closed_note}}", "text": "Состав портфеля"}))];
    v.push(fact("Структура", (9, 0, 5, 4), bind("dumix", &["short"], vec![m("m0", "value", "sum", "", "number")], json!({"filters": [eq("acct", ROW_ACCT)]})), json!({"kpiGroup": "shares", "note": "{names}"}), 13.0));
    v.push(fact("Инструментов", (14, 0, 5, 4), bind("duh", &[], vec![m("m0", "name", "count", "", "number")], json!({"filters": [{"field": "bucket", "op": "!=", "value": "Денежная позиция"}, eq("acct", ROW_ACCT)]})),
        json!({"decimals": 0, "note": "в портфеле · {{row.manager}}"}), 13.0));
    v.push(kpi("value-right", "Стоимость портфеля", (19, 0, 5, 4), bind("acct", &[], vec![m("m0", "value", "sum", "", "currency")], json!({"filters": [eq("acct", ROW_ACCT)]})), json!({})));
    v.push(ink_rule(0, 4, 24));
    v.push(table("grouped", "", (0, 5, 24, 12),
        bind("duh", &["bucket", "name"], vec![
            m("m0", "units", "max", "Кол-во", "number"),
            m("m8", "avg", "max", "Ср. цена ·\nFIFO", "number"),
            m("m1", "price", "max", "Тек. цена", "number"),
            m("m9", "chg", "max", "Изм. цены", "percent"),
            m("m2", "value", "sum", "Стоимость, ₽", "number"),
            m("m7", "received", "sum", "Получено, ₽", "number"),
            m("m3", "income", "sum", "Результат, ₽", "number"),
            m("m4", "ret", "max", "Доходность", "percent"),
            m("m5", "share", "sum", "Доля", "percent"),
            m("m6", "isin", "text", " ", ""),
        ], json!({"filters": [eq("acct", ROW_ACCT)]})),
        json!({"chainKey": "du", "table": {"groupColor": INK, "totalLabel": "Итого портфель", "totalFont": {"size": 8.6, "weight": 800}, "row_h": 22, "paginate": true,
            "cols": {"__tree": {"label": "Инструмент", "sub": "m6", "width": 28, "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "m6": {"hidden": true},
                "m0": {"groupHide": true, "zero": "dash", "decimals": 0, "total": "none"}, "m8": {"groupHide": true, "decimals": 2, "zero": "dash", "total": "none"},
                "m1": {"groupHide": true, "decimals": 2, "zero": "dash", "total": "none"}, "m9": {"signColor": true, "signed": true, "decimals": 1, "groupHide": true, "total": "none"}, "m2": {"bold": true},
                "m7": {"color": "$dividend", "zero": "dash"},
                "m3": {"signColor": true, "bold": true}, "m4": {"signColor": true, "signed": true, "decimals": 1, "total": "none", "groupHide": true}, "m5": {"color": SOFT, "decimals": 1}}}})));
    v.extend(after_table("du", total_bar(ROW_ACCT, "Итого портфель", 18)));
    v
}

/// An insurance policy: value and result, its terms and surrender value, and how both moved.
fn ins_overview() -> Vec<Widget> {
    let mut v = account_header("{{row.badge}}", "{{row.type_name}}{{row.closed_note}}", "Обзор полиса", ROW_ACCT, vec![
        ("Доходность · период", "twr", "percent", "TWR · нарастающим итогом"),
        ("Результат за период", "result", "currency", "гарантия 3% годовых + переоценка"),
    ], "Стоимость полиса");
    v.extend(change_cards(ROW_ACCT, 5, ["Стоимость полиса сейчас", "Взносы и выплаты", "Инвестиционный результат", "Изменение стоимости"], ["Взносы", "Выплаты"], ["Гарантированный доход и комиссии", "Переоценка"]));
    v.extend(account_body(ROW_ACCT, "Полис"));
    // millions in the subtitle, so labels and axis say just 7,4
    v.push(view("bar", "", (12, 10, 12, 6), json!({"title": "Стоимость и выкупная сумма", "subtitle": "на конец месяца · млн ₽",
        "binding": bind("polhist", &["date:month"], vec![m("m0", "value", "sum", "Стоимость", "number"), m("m1", "redeem", "sum", "Выкупная сумма", "number")], json!({"filters": [eq("acct", ROW_ACCT)]})),
        "opts": {"viz": {"units": "millions", "decimals": 1, "labels": "all", "labelColor": "series", "labelPos": "outside", "legend": "top-right", "ticks": 3,
            "colors": {"Стоимость": PRINCIPAL, "Выкупная сумма": INCOME}}}})));
    // the policy's terms as facts: name small above, value below, two across
    v.push(table("card", "Условия полиса", (12, 16, 12, 8), bind("policy", &["key"], vec![m("m0", "value", "text", " ", ""), m("m1", "num", "sum", " ", "currency")], json!({"filters": [eq("acct", ROW_ACCT)]})),
        json!({"table": {"layout": "facts", "factCols": 2, "factSize": 1.12, "hideHead": true, "row_h": 13, "cols": {"m1": {"decimals": 0}}}}))
        .tap(|w| w.style = json!({"fonts": {"title": {"color": SOFT}, "table_cell": {"size": 7.4}}})));
    v
}

fn bank_overview() -> Vec<Widget> {
    let mut v = account_header("Банковские счета", "", "Обзор и доходность", "BANK", vec![
        ("Доходность · период", "twr", "percent", "TWR · нарастающим итогом"),
        ("Результат за период", "result", "currency", "проценты и переоценка металлов"),
    ], "Текущая стоимость");
    v.extend(change_cards("BANK", 5, ["Стоимость счетов сейчас", "Чистые операции", "Инвестиционный результат", "Изменение стоимости"], ["Пополнения", "Выводы"], ["Проценты по вкладам и счетам", "Переоценка металлов (ОМС)"]));
    v.extend(account_body("BANK", "Счета"));
    v.push(class_results("BANK", (12, 10, 12, 7), "Результат по классам активов", "вклад в результат за период · доходность — к вложенному, не TWR"));
    v.push(section("Лучшие и худшие счета", (12, 17, 12, 1), ""));
    v.extend(leaders("BANK", (12, 18, 12, 6), ("Принесли доход", "Переоценка вниз")));
    v
}

fn bank_structure() -> Vec<Widget> {
    let mut v = vec![heading("account", (0, 0, 8, 4), json!({"badge": "Банковские счета", "eyebrow": "", "text": "Структура счетов"}))];
    v.push(fact("Счетов", (8, 0, 5, 4), bind("bank", &[], vec![m("m0", "account", "count", "", "number"), m("m1", "open", "sum", "", "number"), m("m2", "closed", "sum", "", "number")], json!({})),
        json!({"decimals": 0, "note": "{1} активных · {2} закрытых", "noteColor": "ink"}), 13.0));
    v.push(kpi("card-result", "Проценты после НДФЛ", (13, 0, 6, 4), bind("acct", &[], vec![m("m0", "divs", "sum", "", "currency")], json!({"filters": [eq("acct", "BANK")]})), json!({"note": "по вкладам и счетам · в таблице — начисленные, до налога"})));
    v.push(kpi("value-right", "Текущая стоимость", (19, 0, 5, 4), bind("acct", &[], vec![m("m0", "value", "sum", "", "currency")], json!({"filters": [eq("acct", "BANK")]})), json!({})));
    v.push(ink_rule(0, 4, 24));
    v.push(section("Счета и продукты", (0, 5, 24, 1), &format!("{{${}|■}} выделены вклады с закрытием в течение месяца", &LOSS[1..])));
    v.push(table("grouped", "", (0, 6, 24, 13),
        bind("bank", &["group", "account"], vec![
            m("m0", "rate", "max", "Ставка", "percent"),
            m("m1", "term", "text", "Срок · закрытие", ""),
            m("m2", "sum", "sum", "Сумма, ₽", "number"),
            m("m7", "interest", "sum", "Проценты, ₽", "number"),
            m("m3", "result", "sum", "Результат, ₽", "number"),
            m("m4", "ret", "max", "За период", "percent"),
            m("m5", "share", "sum", "Доля", "percent"),
            m("m6", "soon", "max", "Дней до закрытия", "number"),
        ], json!({})),
        json!({"totals": false, "chainKey": "bank", "table": {"groupCount": false, "row_h": 11.5, "head_h": 20,
            "cols": {"__tree": {"label": "Счёт · тип", "width": 30, "icon": {"map": {"Накопительный": "piggy-bank", "Вклад": "percent", "Платёжный": "wallet", "карта": "credit-card", "Карта": "credit-card"}, "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "m0": {"groupHide": true, "decimals": 2, "bold": true, "zero": "dash"}, "m1": {"groupHide": true, "color": SOFT},
                "m2": {"bold": true}, "m7": {"color": "$dividend", "zero": "dash"}, "m3": {"signColor": true, "signed": true}, "m4": {"signColor": true, "signed": true, "decimals": 1, "groupHide": true}, "m5": {"color": SOFT, "decimals": 1},
                "m6": {"hidden": true, "rules": [{"op": "between", "value": "0", "value2": "30", "row": true, "fill": LOSS_TINT, "accent": LOSS, "tag": "{v} дней", "tagAt": "m1", "tagFill": LOSS, "tagColor": PAPER}]}}}}))
        .tap(|w| w.style = json!({"fonts": {"table_cell": {"size": 7.4}}})));
    v.extend(after_table("bank", total_bar("BANK", "Итого банковские счета", 19)));
    v
}

// ---- forecast

fn month_measures(first: Json, last: Json) -> Vec<Json> {
    // the 12 months after the period, named from the data
    let mut v = vec![first];
    for i in 0..12 {
        v.push(m(&format!("m{i}"), &format!("m{}", i + 1), "sum", &format!("{{{{data:port.fc_m{}}}}}", i + 1), "number"));
    }
    v.push(last);
    v
}

fn monthly_receipts() -> Vec<Widget> {
    let mut measures = month_measures(m("mf", "fact", "sum", "Получено\nфакт · {{data:port.fact_range}}", "number"), m("mt", "total", "sum", "Итого\nфакт+12м", "number"));
    measures.push(m("ms", "sub", "text", " ", ""));
    measures.push(m("mk", "kind", "text", " ", ""));
    let mut cols = serde_json::Map::new();
    cols.insert("source".into(), json!({"label": "Источник", "sub": "ms", "width": 17, "swatch": "square", "swatchMap": {"Проценты": INTEREST, "Дивиденды": DIVIDEND, "Купоны": COUPON, "Регулярный": INCOME, "Возврат": PRINCIPAL}}));
    cols.insert("ms".into(), json!({"hidden": true}));
    cols.insert("mk".into(), json!({"hidden": true, "rules": [{"op": "=", "value": "sub", "row": true, "bold": true}]}));
    cols.insert("mf".into(), json!({"decimals": 1, "zero": "dash", "band": TINT_RECEIVED, "bold": true, "bar": {"on": true, "pos": "below", "color": INCOME}, "rule": "right", "headColor": GAIN}));
    for i in 0..12 {
        cols.insert(format!("m{i}"), json!({"decimals": 1, "zero": "dash", "align": "center", "color": FORECAST, "bold": true}));
    }
    cols.insert("mt".into(), json!({"align": "right", "bold": true, "rule": "left", "decimals": 1}));
    let gain_kpi = kpi("card", "Получено (факт)", (0, 3, 6, 4), bind("cfm", &[], vec![m("m0", "fact", "sum", "", "number")], json!({"filters": [eq("kind", "sub")]})), json!({"note": "{{start}} – {{end}} · получено до НДФЛ", "suffix": " млн ₽", "decimals": 1}))
        .tap(|w| w.style = json!({"fonts": {"kpi": {"color": GAIN, "size": 17}}}));
    vec![
        heading("eyebrow", (0, 0, 24, 3), json!({"eyebrow": "Денежный поток · факт и прогноз", "text": "Ежемесячные поступления", "meta": "Получено + помесячный прогноз · млн ₽ · до НДФЛ"})),
        gain_kpi,
        fact("Прогноз · 12 месяцев", (6, 3, 6, 4), bind("fcm", &[], vec![m("m0", "regular", "sum", "", "number")], json!({})), json!({"decimals": 1, "suffix": " млн ₽", "note": "{{data:port.fc_range}}"}), 17.0)
            .tap(|w| w.style = json!({"border": ACCENT_LINE, "borderWidth": 1.1, "fonts": {"kpi": {"color": DIVIDEND, "size": 17}}})),
        fact("Среднемесячный прогноз", (12, 3, 6, 4), bind("fcm", &[], vec![m("m0", "regular", "avg", "", "number")], json!({})), json!({"decimals": 1, "suffix": " млн ₽", "note": "в среднем за 12 месяцев"}), 17.0),
        fact("Пиковый месяц", (18, 3, 6, 4), bind("fcm", &["month"], vec![m("m0", "regular", "sum", "", "number")], json!({})), json!({"kpiGroup": "name", "valueJoin": true, "decimals": 1, "note": "пик регулярного дохода · без возврата капитала"}), 17.0),
        table("matrix", "", (0, 7, 24, 8), bind("cfm", &["source"], measures, json!({})), json!({"table": {"head_h": 26, "row_h": 22, "cols": Json::Object(cols)}})),
        view("pie", "inflow", (0, 16, 7, 8), json!({"title": "Приток за 12 месяцев", "binding": bind("inflow", &["kind"], vec![m("m0", "amount", "sum", "", "number")], json!({})),
            "opts": {"viz": {"palette": [PRINCIPAL, "$incomeSoft"], "centerLabel": "млн ₽"}}})),
        vrule(7, 16, 8),
        table("list", "Топ-5 источников дохода · 12 мес, млн ₽", (8, 16, 8, 8), bind("srcs", &["name"], vec![with(m("m0", "amount", "sum", "#", "number"), json!({"calc": "rank"})), m("m1", "amount", "sum", " ", "number"), m("m2", "kind", "text", " ", "")], json!({})),
            json!({"totals": true, "table": {"order": ["m0", "name", "m1"], "row_h": 13, "totalLabel": "Итого доход", "totalLine": "none", "cols": {
                "m0": {"color": SOFT, "width": 7, "total": "none"}, "name": {"bold": true, "width": 63, "icon": {"map": {"Накопительный": "piggy-bank", "Накопит.": "piggy-bank", "Вклад": "percent"}, "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "m2": {"hidden": true}, "m1": {"decimals": 1, "bold": true, "bar": {"on": true, "pos": "below", "color": SHARE}}}}}))
            .tap(|w| w.style = json!({"fonts": {"table_cell": {"size": 7}}})),
        vrule(16, 16, 8),
        table("list", "Возврат капитала", (17, 16, 7, 8), bind("principal", &["name"], vec![m("m0", "amount", "sum", " ", "number"), m("m1", "date", "text", " ", "")], json!({})),
            json!({"totals": true, "table": {"totalLabel": "Итого возврат", "row_h": 18, "totalLine": "none", "cols": {"name": {"sub": "m1", "bold": true}, "m1": {"hidden": true}, "m0": {"decimals": 1, "bold": true, "bar": {"on": true, "pos": "below", "color": PRINCIPAL}}}}}))
            .tap(|w| {
                // same title and text as the sources beside it; the principal colour stays on the bars
                w.subtitle = "к реинвестированию".into();
                w.style = json!({"fonts": {"table_cell": {"size": 7}}});
            }),
    ]
}

fn monthly_detail() -> Vec<Widget> {
    let mut measures = month_measures(m("mf", "fact", "sum", "Получено\nфакт", "number"), m("mt", "total", "sum", "Итого\nфакт+12м", "number"));
    measures.push(m("mk", "kind", "text", " ", ""));
    let mut cols = serde_json::Map::new();
    cols.insert("name".into(), json!({"label": "Инструмент / счёт · тип", "width": 30, "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}));
    cols.insert("mk".into(), json!({"hidden": true, "rules": [
        {"op": "=", "value": "gd", "row": true, "bold": true, "color": INTEREST},
        {"op": "=", "value": "gv", "row": true, "bold": true, "color": DIVIDEND},
        {"op": "=", "value": "gc", "row": true, "bold": true, "color": COUPON},
        {"op": "=", "value": "gr", "row": true, "bold": true, "color": PRINCIPAL}
    ]}));
    cols.insert("mf".into(), json!({"decimals": 0, "zero": "dash", "align": "center", "band": TINT_RECEIVED, "bold": true, "rule": "right", "headColor": GAIN}));
    for i in 0..12 {
        cols.insert(format!("m{i}"), json!({"decimals": 0, "zero": "dash", "align": "center", "color": FORECAST}));
    }
    cols.insert("mt".into(), json!({"decimals": 0, "bold": true, "rule": "left"}));
    vec![
        heading("eyebrow", (0, 0, 24, 3), json!({"eyebrow": "График поступлений · по месяцам", "text": "Детализация поступлений", "meta": "По инструментам · тыс ₽ · до НДФЛ"})),
        fact("Получено (факт)", (0, 3, 6, 4), bind("cfm", &[], vec![m("m0", "fact", "sum", "", "number")], json!({"filters": [eq("kind", "sub")]})), json!({"decimals": 1, "suffix": " млн ₽", "note": "{{start}} – {{end}} · до НДФЛ"}), 17.0)
            .tap(|w| w.style = json!({"fonts": {"kpi": {"color": GAIN, "size": 17}}})),
        fact("Регулярный доход · 12 мес", (6, 3, 6, 4), bind("fcm", &[], vec![m("m0", "regular", "sum", "", "number")], json!({})), json!({"decimals": 1, "suffix": " млн ₽", "note": "{{data:port.fc_range}} · прогноз"}), 17.0)
            .tap(|w| w.style = json!({"border": ACCENT_LINE, "borderWidth": 1.1, "fonts": {"kpi": {"size": 17, "color": INCOME}}})),
        fact("Возврат капитала", (12, 3, 6, 4), bind("principal", &[], vec![m("m0", "amount", "sum", "", "number"), m("m1", "name", "count", "позиций", "number")], json!({})), json!({"decimals": 1, "suffix": " млн ₽", "note": "{{data:principal.note}}", "noteColor": "ink"}), 17.0)
            .tap(|w| w.style = json!({"fonts": {"kpi": {"color": PRINCIPAL, "size": 17}}})),
        fact("Ближайшее закрытие", (18, 3, 6, 4), bind("principal", &["day"], vec![m("m0", "amount", "sum", "", "number"), m("m1", "name", "text", "", "")], json!({"sort": "dim_asc"})), json!({"kpiGroup": "name", "valueJoin": true, "decimals": 1, "note": "{1} · млн ₽"}), 17.0),
        // a client with many payments: the table goes on to the next page rather than hide rows
        table("matrix", "", (0, 7, 24, 16), bind("cfi", &["name"], measures, json!({})), json!({"table": {"head_h": 22, "row_h": 13.5, "paginate": true, "cols": Json::Object(cols)}}))
            .tap(|w| w.style = json!({"fonts": {"table_cell": {"size": 6.8}}})),
        text("legend", (0, 23, 24, 1), &format!("{{${}|■}} Получено · факт ({{{{start}}}} – {{{{end}}}})     ■ Прогноз · {{{{data:port.fc_range}}}}     |     {{${}|■}} Вклады   {{${}|■}} Дивиденды   {{${}|■}} Купоны   {{${}|■}} Возврат капитала",
            &INCOME[1..], &INTEREST[1..], &DIVIDEND[1..], &COUPON[1..], &PRINCIPAL[1..]), json!({"align": "center"})),
    ]
}

// ---- activity

/// The period's activity: four summary cards (each a total and its two parts),
/// then the largest items under them. Buying and selling are neither good nor
/// bad: their parts are drawn in the theme's soft tones, not green and red.
fn trades_overview() -> Vec<Widget> {
    let port = |measures: Vec<Json>| bind("port", &[], measures, json!({}));
    // a total with its two parts as rows under it
    let summary = |title: &str, x: u32, measures: Vec<Json>, palette: Json, signed: bool| {
        kpi("breakdown", title, (x, 3, 6, 5), port(measures), json!({"breakdown": true, "breakdownTone": "neutral", "palette": palette, "compact": "on",
            "valueColor": if signed { "sign" } else { "font" }, "signed": signed, "decimals": 1}))
            .tap(|w| w.style = json!({"fonts": {"kpi": {"size": 17}}}))
    };
    let mn = |id: &str, field: &str, label: &str| m(id, field, "sum", label, "currency");
    // buy / sell and in / out badges; deposits and taxes carry their icon by their name already
    let soft_dir = json!({"map": {"Продажа": "sell", "Покупка": "buy", "Ввод": "in", "Вывод": "out"}, "shape": "circle", "size": 0.7, "fill": CHIP, "color": ICON_INK,
        "fillMap": {"Продажа": "$lossMid", "Покупка": "$gainMid", "Ввод": "$gainMid", "Вывод": "$lossMid"}});
    // plain tables under the cards: thin lines, no card, no totals
    let list = |title: &str, rect: (u32, u32, u32, u32), b: Json, cols: Json, order: Json, empty: &str| {
        table("lines", title, rect, b, json!({"totals": false, "emptyText": empty, "table": {"order": order, "cols": cols, "row_h": 19, "headLineWidth": 0.8}}))
    };
    let amount = |decimals: u32| json!({"bold": true, "suffix": " млн", "decimals": decimals});
    // an operation's icon: a deposit, money in or out, a tax
    let ops_icons = with(icon_map(), json!({"Ввод средств": "wallet", "Вывод средств": "wallet", "НДФЛ": "doc"}));
    vec![
        heading("account", (0, 0, 24, 3), json!({"badge": "Обзор", "badgeSize": 8, "eyebrow": "За период", "text": "Сделки и операции", "meta": "Активность · {{start}} — {{end}}", "nav": false, "rule": 1.5})).tap(|w| w.style = json!({"fonts": {"heading": {"size": 18}, "eyebrow": {"color": INK, "size": 7}}})),
        summary("Оборот по сделкам", 0, vec![mn("m0", "turnover", ""), mn("m1", "buys", "Покупки"), mn("m2", "sells", "Продажи")], json!(["$gainMid", "$lossMid"]), false)
            .tap(|w| w.subtitle = "{{data:port.deals_txt}}".into()),
        summary("Валютные конверсии", 6, vec![mn("m0", "fx_total", ""), mn("m1", "fx_buy", "Покупка валюты"), mn("m2", "fx_sell", "Продажа валюты")], json!(["$gainMid", "$lossMid"]), false)
            .tap(|w| w.subtitle = "{{data:port.fx_txt}}".into()),
        summary("Чистый приток средств", 12, vec![mn("m0", "net_in", ""), mn("m1", "cash_in", "Ввод"), mn("m2", "cash_out", "Вывод")], json!(["$gainMid", "$lossMid"]), true)
            .tap(|w| w.subtitle = "{{data:port.cash_txt}}".into()),
        summary("Налоги и комиссии", 18, vec![mn("m0", "charges", ""), mn("m1", "tax_paid", "НДФЛ"), mn("m2", "fees_paid", "Комиссии")], json!(["$coupon", "$couponSoft"]), false)
            .tap(|w| w.subtitle = "{{data:port.charges_txt}}".into()),
        ink_rule(0, 8, 24),
        // what made the totals: the largest items
        list("Крупнейшие сделки", (0, 9, 12, 8), bind("bigtrades", &["name"], vec![m("m0", "acct", "text", "Счёт", ""), m("m2", "dir", "text", "Направление", ""), m("m4", "sum", "sum", "Сумма", "number")], json!({})),
            json!({"name": {"label": "Инструмент", "bold": true, "width": 40, "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "m0": {"color": SOFT}, "m2": {"icon": soft_dir}, "m4": amount(2)}),
            json!(["name", "m0", "m2", "m4"]), "Сделок за период не было"),
        list("Крупнейшие конверсии", (0, 17, 12, 7), bind("conv", &["n"], vec![m("m5", "currency", "text", "Валюта", ""), m("m0", "rate", "max", "Курс", "number"), m("m1", "dir", "text", "Направление", ""), m("m3", "sum", "sum", "Сумма", "number")], json!({})),
            json!({"n": {"hidden": true}, "m5": {"bold": true, "align": "left", "label": "Пара", "icon": {"map": icon_map(), "size": 0.7, "shape": "circle"}}, "m0": {"decimals": 2, "color": SOFT}, "m1": {"icon": soft_dir}, "m3": amount(2)}),
            json!(["m5", "m0", "m1", "m3"]), "Конверсий за период не было"),
        vrule(12, 9, 15),
        // money in and out, and deposits opened and closed: the largest of them
        list("Движение средств и вклады", (13, 9, 11, 9), bind("ops", &["name"], vec![m("ma", "acct", "text", "Счёт", ""), m("m0", "dir", "text", "Направление", ""), m("m1", "sum", "sum", "Сумма", "number")],
                json!({"filters": [{"field": "kind", "op": "in", "value": "cash,dep"}], "sort": "data", "limit": 5})),
            json!({"name": {"label": "Операция", "bold": true, "icon": {"map": ops_icons.clone(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "ma": {"color": SOFT}, "m0": {"icon": soft_dir}, "m1": {"bold": true, "signed": true, "signColor": true, "suffix": " млн", "decimals": 1}}),
            json!(["name", "ma", "m0", "m1"]), "Вводов, выводов и вкладов за период не было"),
        list("Налоги и комиссии", (13, 18, 11, 6), bind("ops", &["name"], vec![m("ma", "acct", "text", "Счёт", ""), m("m0", "dir", "text", "Начисление", ""), m("m1", "sum", "sum", "Сумма", "number")], json!({"filters": [eq("kind", "tax")], "sort": "data", "limit": 3})),
            json!({"name": {"label": "Начисление", "bold": true, "icon": {"map": ops_icons.clone(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}}, "ma": {"color": SOFT}, "m0": {"icon": soft_dir}, "m1": amount(2)}),
            json!(["name", "ma", "m0", "m1"]), "Налогов и комиссий за период не было"),
    ]
}

fn trades_detail() -> Vec<Widget> {
    let flows = |filters: Vec<Json>| bind("flows", &[], vec![m("m0", "n", "sum", "", "number"), m("m1", "amount", "sum", "", "number")], json!({"filters": filters}));
    vec![
        heading("account", (0, 0, 12, 4), json!({"badge": "Детали", "eyebrow": "По инструментам", "text": "Сделки по инструментам", "subtitle": "Активность · крупнейшие сделки периода", "nav": false})),
        // count of deals · amount, from the trade totals
        fact("Покупки", (12, 0, 4, 4), flows(vec![eq("kind", "trades"), eq("part", "Покупки")]), json!({"valueJoin": true, "decimals": 1, "suffix": " млн", "note": "Открытие и наращивание"}), 13.0),
        fact("Продажи", (16, 0, 4, 4), flows(vec![eq("kind", "trades"), eq("part", "Продажи")]), json!({"valueJoin": true, "decimals": 1, "suffix": " млн", "note": "Фиксация и ребаланс"}), 13.0),
        fact("Конверсии", (20, 0, 4, 4), flows(vec![eq("kind", "fx")]), json!({"valueJoin": true, "decimals": 1, "suffix": " млн", "note": "USD/RUB · CNY/RUB"}), 13.0),
        ink_rule(0, 4, 24),
        table("lines", "", (0, 5, 24, 16),
            bind("deals", &["name"], vec![m("mk", "kind", "text", "Тип", ""), m("m0", "acct", "text", "Счёт", ""), m("m1", "type", "text", "Направление", ""), m("m2", "date", "text", "Дата", ""), m("m3", "qty", "text", "Кол-во", ""), m("m4", "price", "max", "Цена, ₽", "number"), m("m5", "sum", "sum", "Сумма, ₽", "number"), m("m6", "fee", "sum", "Комис., ₽", "number"),
                m("m7", "res_pct", "max", "Итог на конец", "number")], json!({})),
            json!({"table": {"totalLabel": "Итого · {n} крупнейших сделок", "row_h": 21, "cols": {
                "name": {"label": "Инструмент", "bold": true, "width": 22, "icon": {"map": icon_map(), "auto": true, "size": 0.7, "shape": "circle", "fill": CHIP, "color": ICON_INK}},
                // the kind of instrument as a pill: shares, bonds, funds, currency
                "mk": {"align": "center", "total": "none", "pill": {"on": true, "fill": CHIP, "color": ICON_INK}},
                "m0": {"color": SOFT, "total": "none"},
                "m1": {"icon": {"map": {"Продажа": "sell", "Покупка": "buy"}, "shape": "circle", "size": 0.7, "fillMap": {"Продажа": "$lossMid", "Покупка": "$gainMid"}}, "total": "none"},
                "m2": {"align": "right", "color": SOFT}, "m3": {"align": "right", "bold": true}, "m4": {"decimals": 2, "total": "none", "zero": "dash"},
                "m5": {"bold": true, "signColor": true, "signed": true, "total": "none"}, "m6": {"color": SOFT},
                // how the deal looks at the period's end (a sale: against the average cost before it)
                "m7": {"bold": true, "signColor": true, "signed": true, "decimals": 1, "suffix": "%", "total": "none"}}}})),
        text("small", (0, 21, 24, 1), "{{data:port.deals_note}}. Итог на конец: покупка — к цене на {{end}}, продажа — к средней цене покупки, конверсия — к курсу; без комиссий и купонов.", json!({})),
    ]
}

/// Money in and out of the accounts, and the taxes and fees paid: each payment, and the charges by account.
fn money_detail() -> Vec<Widget> {
    let port = |field: &str| bind("port", &[], vec![m("m0", field, "sum", "", "currency")], json!({}));
    let card = |label: &str, x: u32, field: &str, note: &str, signed: bool| {
        fact(label, (x, 0, 4, 4), port(field), json!({"compact": "on", "note": note, "valueColor": if signed { "sign" } else { "font" }, "signed": signed}), 13.0)
    };
    let dir = json!({"map": {"Ввод": "in", "Вывод": "out"}, "shape": "circle", "size": 0.7, "fill": CHIP, "color": ICON_INK, "fillMap": {"Ввод": "$gainMid", "Вывод": "$lossMid"}});
    let charges = |title: &str, rect: (u32, u32, u32, u32), kind: &str, empty: &str| {
        table("lines", title, rect, bind("charges", &["name", "acct"], vec![m("m0", "n", "sum", "Шт.", "number"), m("m1", "paid", "sum", "Сумма", "compact")], json!({"filters": [eq("kind", kind)]})),
            json!({"emptyText": empty, "table": {"totalLabel": "Итого", "row_h": 19, "headLineWidth": 0.8, "cols": {
                "name": {"label": "Вид", "bold": true, "width": 26}, "acct": {"label": "Счёт", "color": SOFT},
                "m0": {"align": "right", "color": SOFT}, "m1": {"bold": true, "align": "right"}}}}))
    };
    vec![
        heading("account", (0, 0, 8, 4), json!({"badge": "Детали", "eyebrow": "Движение средств", "text": "Деньги и сборы", "subtitle": "Вводы, выводы, НДФЛ и комиссии", "nav": false})),
        card("Введено", 8, "cash_in", "Пополнения счетов", false),
        card("Выведено", 12, "cash_out", "Выводы со счетов", false),
        card("НДФЛ", 16, "tax_paid", "Удержан с дохода", false),
        card("Комиссии", 20, "fees_paid", "Сделки, обслуживание, управление", false),
        ink_rule(0, 4, 24),
        table("lines", "Вводы и выводы", (0, 5, 13, 16),
            bind("cashops", &["n"], vec![m("m0", "date", "text", "Дата", ""), m("m1", "acct", "text", "Счёт", ""), m("m2", "dir", "text", "Операция", ""), m("m3", "sum", "sum", "Сумма, млн ₽", "number")], json!({})),
            json!({"emptyText": "Вводов и выводов за период не было", "table": {"totalLabel": "Итого · {n} операций", "row_h": 19, "headLineWidth": 0.8, "order": ["m0", "m1", "m2", "m3"], "cols": {
                "n": {"hidden": true}, "m0": {"color": SOFT, "total": "none", "align": "left"}, "m1": {"bold": true, "total": "none", "align": "left"}, "m2": {"icon": dir, "total": "none"},
                "m3": {"bold": true, "signed": true, "signColor": true, "decimals": 2, "align": "right"}}}})),
        vrule(13, 5, 16),
        charges("Налоги", (14, 5, 10, 7), "tax", "Налогов за период не было"),
        charges("Комиссии", (14, 12, 10, 9), "fee", "Комиссий за период не было"),
        text("small", (0, 21, 24, 1), "Вводы и выводы — до 14 крупнейших, по дате; все — в карточках выше. НДФЛ — с купонов, дивидендов и процентов. Комиссии по сделкам — брокерская и депозитарная.", json!({})),
    ]
}

fn legal() -> Vec<Widget> {
    let a = "Информация, изложенная в настоящем отчёте, подготовлена специалистами Test Private Banking с единственной целью — предоставить сводную информацию общего характера в отношении активов на Ваших счетах в компаниях группы и у её партнёров. Отчёт адресован и направляется только Вам; иные лица не вправе действовать на основании настоящего документа или полагаться на него.\n\nНастоящий отчёт **не** предназначен для обоснования инвестиционного решения, **не** является официальным источником отчётности и **не** должен рассматриваться как финансовая рекомендация или инвестиционный совет.\n\nОтчёт подготовлен на основе материалов, полученных от компаний группы и партнёров. Test Private Banking не предоставляет заверений в полноте и достоверности этой информации, и она не должна рассматриваться как полная и достоверная.\n\nНи Test Private Banking, ни компании группы, ни аффилированные лица и партнёры не несут ответственности за точность, достоверность и полноту содержащейся в отчёте информации, а также за отсутствие какой-либо информации.\n\nДля получения официальной отчётности Вам необходимо обратиться в соответствующую компанию группы или к партнёру в порядке, предусмотренном договором и действующим законодательством.";
    let b = "Настоящий отчёт не является предложением о продаже ценных бумаг или подписке на них, а также рекламой ценных бумаг ни по какому применимому законодательству и ни в какой юрисдикции.\n\nTest Private Banking и компании группы не обязуются предоставлять доступ к дополнительной информации, обновлять отчёт или исправлять неточности, которые впоследствии могут стать очевидными.\n\nОтчёт и его содержание являются конфиденциальной информацией, подлежат рассмотрению в строгой конфиденциальности и не подлежат копированию, воспроизведению, распространению или раскрытию другим лицам без предварительного письменного согласия Test Private Banking.";
    let c = "**Тест Банк (ПАО)** · Генеральная лицензия на осуществление банковских операций № 1481.\n117997, Москва, ул. Вавилова, д. 19 · +7 (495) 500-55-50\n\n**Тест Капитал (АО)** · Лицензии профучастника рынка ценных бумаг на брокерскую, дилерскую и депозитарную деятельность.\n\n**УК «Капитал» (АО)** · Лицензия на деятельность по управлению инвестиционными и паевыми инвестиционными фондами.";
    vec![
        heading("page", (0, 0, 24, 3), json!({"text": "Юридическая информация", "meta": "Конфиденциально · только для адресата"})).tap(|w| w.style = json!({"fonts": {"heading": {"size": 30}}})),
        text("body", (0, 3, 11, 18), a, json!({})),
        vrule(11, 3, 20),
        text("body", (12, 3, 12, 10), b, json!({})),
        view("text", "body", (12, 16, 12, 7), json!({"title": "Реквизиты и лицензии", "opts": {"text": c},
            "style": {"card": true, "background": CARD, "fonts": {"title": {"size": 5.8, "weight": 700, "color": SOFT, "tracking": 0.1}, "body": {"size": 6.8, "color": BODY}}}})),
    ]
}

#[cfg(test)]
mod divider_tests {
    use crate::model::effective;

    #[test]
    fn halves_meet_without_a_divider() {
        let mut doc = super::template();
        // no vertical dividers left between halves
        let vertical: Vec<String> = doc.pages.iter().flat_map(|p| p.widgets.iter()
            .filter(|w| w.kind == "divider" && effective(&doc.theme, w).opt_bool("vertical") == Some(true))
            .map(move |w| format!("{}@{}", p.title, w.x))).collect();
        assert!(vertical.is_empty(), "vertical dividers left: {vertical:?}");
        // halves of 12 and 12 on the overview; the forecast page's three blocks of 8
        let edges = |title: &str| -> Vec<(u32, u32)> {
            let p = doc.pages.iter().find(|p| p.title == title).unwrap();
            let mut e: Vec<(u32, u32)> = p.widgets.iter().filter(|w| w.y >= 5 && w.w < 24).map(|w| (w.x, w.x + w.w)).collect();
            e.sort();
            e.dedup();
            e
        };
        assert!(edges("Обзор портфеля").iter().all(|&(x, r)| r <= 12 || x >= 12), "the overview's halves meet at column 12: {:?}", edges("Обзор портфеля"));
        let f = edges("Факт и прогноз на 12 месяцев");
        assert!(f.contains(&(0, 8)) && f.contains(&(8, 16)) && f.contains(&(16, 24)), "the forecast's thirds are 8, 8 and 8: {f:?}");
        assert_eq!(doc.remove_split_dividers(), 0, "removing twice changes nothing");
    }
}

#[cfg(test)]
mod series_axes_tests {
    #[test]
    fn monthly_and_running_totals_use_two_axes() {
        for (name, mut doc) in [("uhnwi", super::template()), ("client", crate::client_sample::template())] {
            let charts: Vec<_> = doc.pages.iter().flat_map(|p| &p.widgets)
                .filter(|w| w.kind == "bar" && w.binding.as_ref().is_some_and(|b| b.measures.len() == 2 && b.measures.iter().any(|m| m.chart.as_deref() == Some("line"))))
                .collect();
            assert!(!charts.is_empty(), "{name}: has charts of months and their total");
            for w in &charts {
                let b = w.binding.as_ref().unwrap();
                let line = b.measures.iter().find(|m| m.chart.as_deref() == Some("line")).unwrap();
                let col = b.measures.iter().find(|m| m.chart.as_deref() != Some("line")).unwrap();
                let so = &w.opts["viz"]["seriesOpts"];
                assert_eq!((so[&line.id]["axis"].as_str(), so[&line.id]["labels"].as_str()), (Some("right"), Some("last")), "{name}: {} — the total", w.title);
                assert_eq!((so[&col.id]["labels"].as_str(), so[&col.id]["labelPos"].as_str()), (Some("all"), Some("outside")), "{name}: {} — the months", w.title);
            }
            assert_eq!(doc.split_series_axes(), 0, "{name}: a chart set per series is left as it is");
        }
    }
}
