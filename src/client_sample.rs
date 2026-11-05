//! A private-banking client report template in Russian: cover, contents,
//! portfolio overview, results, benchmarks, holdings, cash flow and legal
//! pages, with generated data. It exercises the features such reports need:
//! locale formatting, section tabs, data placeholders, treemaps, waterfalls,
//! combo charts, tiles, grouped and paginated tables and pivots.

use crate::model::*;
use crate::period::ymd;
use serde_json::{json, Value as Json};

struct Pos {
    name: &'static str,
    isin: &'static str,
    provider: &'static str,
    account: &'static str,
    product: &'static str,
    class: &'static str,
    currency: &'static str,
    qty: f64,
    avg: f64,
    price: f64,
    value: f64,
    income: f64,
    divs: f64,
}

const DEP: &str = "Депозиты, средства и эквиваленты";
const BRK: &str = "Брокерские счета";
const PIF: &str = "Паевые Инвестиционные Фонды";
const OMS: &str = "Обезличенные металлические счета";
const DU: &str = "Доверительное управление";

#[rustfmt::skip]
fn positions() -> Vec<Pos> {
    let p = |name, isin, provider, account, product, class, currency, qty, avg, price, value, income, divs| Pos { name, isin, provider, account, product, class, currency, qty, avg, price, value, income, divs };
    vec![
        p("Накопительный счёт (руб.)", "", "Тест Банк · денежные средства", "***23990", DEP, "Денежные средства", "RUB", 0.0, 0.0, 0.0, 210_400_000.0, 16_000_000.0, 0.0),
        p("Вклад «Премиум»", "", "Тест Банк · денежные средства", "***52210", DEP, "Денежные средства", "RUB", 0.0, 0.0, 0.0, 63_200_000.0, 6_200_000.0, 0.0),
        p("Вклад «Доходный»", "", "Тест Банк · денежные средства", "***44021", DEP, "Денежные средства", "RUB", 0.0, 0.0, 0.0, 74_000.0, 7_114_000.0, 0.0),
        p("БПИФ «Капитал – Топ российских акций»", "ISIN RU000A104172", "Брокерский счёт 400PX0F · фонды", "400PX0F", BRK, "Акции", "RUB", 1_250_000.0, 16.74, 14.04, 17_556_000.0, -3_365_000.0, 0.0),
        p("Тест Банк, ао", "ISIN RU000A0J2Q06", "Брокерский счёт 400PX0F · акции", "400PX0F", BRK, "Акции", "RUB", 22_000.0, 290.77, 309.86, 6_817_000.0, 1_120_000.0, 700_000.0),
        p("ММК, ао", "ISIN RU0009084396", "Брокерский счёт 400PX0F · акции", "400PX0F", BRK, "Акции", "RUB", 120_000.0, 38.75, 30.58, 3_670_000.0, -770_000.0, 210_000.0),
        p("Инарктика, ао", "ISIN RU000A0JR514", "Брокерский счёт 400PX0F · акции", "400PX0F", BRK, "Акции", "RUB", 3_800.0, 644.21, 496.84, 1_888_000.0, -465_000.0, 95_000.0),
        p("Хэндерсон, ао", "ISIN RU000A1075S4", "Брокерский счёт 400PX0F · акции", "400PX0F", BRK, "Акции", "RUB", 3_200.0, 580.0, 445.63, 1_426_000.0, -366_000.0, 64_000.0),
        p("Аэрофлот, ао", "ISIN RU0009062285", "Брокерский счёт 400PX0F · акции", "400PX0F", BRK, "Акции", "RUB", 27_000.0, 58.41, 50.63, 1_367_000.0, -210_000.0, 0.0),
        p("БПИФ «Капитал – Сберегательный в юанях»", "ISIN RU000A105L84", "Брокерский счёт 400PX0F · фонды", "400PX0F", BRK, "Акции", "CNY", 20_000.0, 10.95, 11.5, 230_000.0, 11_000.0, 0.0),
        p("Банк ВТБ, ао", "ISIN RU000A0JP5V6", "Брокерский счёт 400PX0F · акции", "400PX0F", BRK, "Акции", "RUB", 1_750.0, 95.43, 90.86, 159_000.0, -8_000.0, 0.0),
        p("ОФЗ-26221-ПД", "ISIN RU000A0JX0J2 · погашение 23.03.2033 · купон 7,70%", "Брокерский счёт 400PX0F · облигации", "400PX0F", BRK, "Облигации", "RUB", 1_600.0, 974.0, 930.0, 1_488_000.0, -12_000.0, 58_000.0),
        p("ОФЗ-26240-ПД", "ISIN RU000A102HG2 · погашение 30.07.2036 · купон 7,00%", "Брокерский счёт 400PX0F · облигации", "400PX0F", BRK, "Облигации", "RUB", 950.0, 719.0, 672.0, 638_000.0, -23_000.0, 22_000.0),
        p("ОФЗ-26238-ПД", "ISIN RU000A1038V6 · погашение 15.05.2041 · купон 7,10%", "Брокерский счёт 400PX0F · облигации", "400PX0F", BRK, "Облигации", "RUB", 1_050.0, 572.0, 539.0, 566_000.0, -16_000.0, 19_000.0),
        p("Золото (обезличенный металл)", "Обезличенный металлический счёт", "Брокерский счёт 400PX0F · металлы", "400PX0F", BRK, "Драгметаллы", "Прочие", 970.0, 9_652.0, 9_941.0, 9_643_000.0, 280_000.0, 0.0),
        p("Российский рубль", "Валюта счёта · RUB", "Брокерский счёт 400PX0F · деньги", "400PX0F", BRK, "Денежные средства", "RUB", 0.0, 0.0, 0.0, 1_971_000.0, 0.0, 0.0),
        p("Доллар США", "Валюта счёта · USD", "Брокерский счёт 400PX0F · деньги", "400PX0F", BRK, "Денежные средства", "USD", 0.0, 0.0, 0.0, 7_086_000.0, -1_150_000.0, 0.0),
        p("Структурная нота «Рынок+»", "ISIN XS2390417", "Брокерский счёт 400PX0F · прочие", "400PX0F", BRK, "Прочие", "Прочие", 6.0, 1_050_000.0, 1_152_500.0, 6_915_000.0, 980_000.0, 0.0),
        p("Фонд российских акций", "", "УК Капитал · ПИФ", "ПИФ", PIF, "Фонды", "RUB", 0.0, 0.0, 0.0, 12_400_000.0, -1_200_000.0, 0.0),
        p("Фонд облигаций «Сбалансированный»", "", "УК Капитал · ПИФ", "ПИФ", PIF, "Фонды", "RUB", 0.0, 0.0, 0.0, 6_269_000.0, -357_000.0, 0.0),
        p("ОМС · золото", "", "Тест Банк · драгметаллы", "***19623", OMS, "Драгметаллы", "Прочие", 0.0, 0.0, 0.0, 5_200_000.0, -800_000.0, 0.0),
        p("ОМС · серебро", "", "Тест Банк · драгметаллы", "***13055", OMS, "Драгметаллы", "Прочие", 0.0, 0.0, 0.0, 1_788_000.0, -464_000.0, 0.0),
        p("ОМС · платина", "", "Тест Банк · драгметаллы", "***03446", OMS, "Драгметаллы", "Прочие", 0.0, 0.0, 0.0, 393_000.0, -730_000.0, 0.0),
        p("БПИФ «Капитал – Топ российских акций» · ДУ", "", "УК Капитал · ДУ", "DU.256730", DU, "Фонды", "RUB", 0.0, 0.0, 0.0, 5_398_000.0, -235_000.0, 0.0),
        p("БПИФ «Капитал – Корпоративные облигации» · ДУ", "", "УК Капитал · ДУ", "DU.256730", DU, "Облигации", "RUB", 0.0, 0.0, 0.0, 1_517_000.0, -15_000.0, 0.0),
    ]
}

pub(crate) const CLIENT: &str = "Петров Пётр Петрович";

pub(crate) fn month_ends() -> Vec<chrono::NaiveDate> {
    vec![ymd(2026, 1, 31), ymd(2026, 2, 28), ymd(2026, 3, 31), ymd(2026, 4, 30), ymd(2026, 5, 31), ymd(2026, 6, 15)]
}

pub(crate) fn positions_table() -> (Vec<String>, Vec<Vec<Json>>) {
    // share of the year's result earned by each month-end (cumulative)
    let path = [0.341, 0.585, 0.871, 0.576, 0.889, 1.0];
    let mut rows = Vec::new();
    for (k, d) in month_ends().into_iter().enumerate() {
        for p in positions() {
            let inc = p.income * path[k];
            let start = p.value - p.income;
            let value = start + inc;
            let chg = if p.avg > 0.0 { p.price / p.avg - 1.0 } else { 0.0 };
            rows.push(vec![
                json!(d.format("%Y-%m-%d").to_string()),
                json!(CLIENT),
                json!(p.account),
                json!(p.product),
                json!(p.class),
                json!(p.currency),
                json!(p.name),
                json!(p.isin),
                json!(p.provider),
                if p.qty > 0.0 { json!(p.qty) } else { Json::Null },
                if p.avg > 0.0 { json!(p.avg) } else { Json::Null },
                if p.price > 0.0 { json!(p.price) } else { Json::Null },
                if p.avg > 0.0 { json!(chg) } else { Json::Null },
                json!(value.round()),
                json!(inc.round()),
                json!((p.divs * path[k]).round()),
                json!(start.round()),
            ]);
        }
    }
    let cols = [
        "date", "client", "account", "product", "asset_class", "currency", "instrument", "isin", "provider", "qty", "avg_price", "price",
        "price_chg", "value", "income", "dividends", "start_value",
    ];
    (cols.iter().map(|s| s.to_string()).collect(), rows)
}

pub(crate) fn inline(id: &str, name: &str, columns: &[&str], rows: Vec<Vec<Json>>, date_field: Option<&str>, snapshot: bool) -> Dataset {
    Dataset {
        id: id.into(),
        name: name.into(),
        source: Source::Inline { columns: columns.iter().map(|s| s.to_string()).collect(), rows },
        date_field: date_field.map(String::from),
        snapshot,
        ..Default::default()
    }
}

fn w(v: Json) -> Widget {
    serde_json::from_value(v).expect("sample widget")
}

/// A measure: field, aggregation, label, format and optional extras.
fn m(id: &str, field: &str, agg: &str, label: &str, format: &str) -> Json {
    json!({"id": id, "field": field, "agg": agg, "label": label, "format": format})
}

fn with(mut a: Json, extra: Json) -> Json {
    if let (Some(o), Some(e)) = (a.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            o.insert(k.clone(), v.clone());
        }
    }
    a
}

fn kpi(title: &str, rect: (u32, u32, u32, u32), binding: Json, opts: Json, style: Json) -> Widget {
    w(json!({"kind": "kpi", "title": title, "x": rect.0, "y": rect.1, "w": rect.2, "h": rect.3, "binding": binding, "opts": opts, "style": style}))
}

fn bind(ds: &str, dims: &[&str], measures: Vec<Json>, extra: Json) -> Json {
    with(json!({"dataset": ds, "dims": dims, "measures": measures, "window": "period", "filters": []}), extra)
}

pub fn template() -> Doc {
    let mut doc = Doc::default();
    doc.meta.title = "Отчёт по активам и доходности".into();
    doc.meta.subtitle = "Персональный отчёт".into();
    doc.meta.organization = "Test Private Banking".into();
    doc.meta.description = "Частное банковское обслуживание: портфель, результаты, ориентиры, состав счетов, денежный поток.".into();

    let th = &mut doc.theme;
    th.locale = "ru-RU".into();
    th.currency = "₽".into();
    th.page_bg = "#f4efe6".into();
    th.card_bg = "#ede5d8".into();
    th.ink = "#1f1d1a".into();
    th.muted = "#857c6f".into();
    th.grid = "#e2d9c9".into();
    th.good = "#1f7a5c".into();
    th.bad = "#b23b3b".into();
    th.card = "filled".into();
    th.chart_look = "soft".into();
    th.radius = 7.0;
    th.palette = ["#5b7aa6", "#b9734f", "#4f9a8a", "#c9a227", "#8e7cb4", "#9a8a72", "#c25b5b", "#4f8a52"].iter().map(|s| s.to_string()).collect();
    th.fonts.insert("heading".into(), FontSpec { family: Some("Georgia".into()), weight: Some(400), ..Default::default() });
    th.fonts.insert("title".into(), FontSpec { size: Some(8.5), weight: Some(700), upper: Some(true), ..Default::default() });
    th.fonts.insert("caption".into(), FontSpec { size: Some(7.0), ..Default::default() });
    th.fonts.insert("table_head".into(), FontSpec { size: Some(6.8), upper: Some(true), ..Default::default() });

    doc.page.rows = 14;
    doc.page.header = Band { show: true, height: 30.0, tabs: "left".into(), right: Slot { text: "{{data:Клиенты.client}}".into(), font: FontSpec { size: Some(7.5), weight: Some(600), ..Default::default() }, ..Default::default() }, ..Default::default() };
    doc.page.footer = Band {
        show: true,
        height: 20.0,
        line: "top".into(),
        left: Slot { text: "{{data:Клиенты.banker}} · {{data:Клиенты.email}} · {{data:Клиенты.phone}}".into(), ..Default::default() },
        center: Slot { text: "Период {{start}} — {{end}} · сформирован {{today}}".into(), ..Default::default() },
        right: Slot { text: "{{page}}".into(), ..Default::default() },
        ..Default::default()
    };

    doc.params.grain = "custom".into();
    doc.params.start = Some(ymd(2026, 1, 1));
    doc.params.date = ymd(2026, 6, 15);
    doc.params.compare = "none".into();
    doc.param_defs = vec![ParamDef { name: "client".into(), label: "Клиент".into(), options: vec![CLIENT.into()], default: CLIENT.into(), ..Default::default() }];
    doc.params.values.insert("client".into(), CLIENT.into());

    // ---- data
    let (pc, pr) = positions_table();
    let pcols: Vec<&str> = pc.iter().map(|s| s.as_str()).collect();
    doc.datasets.push(inline("pos", "Позиции", &pcols, pr, Some("date"), true));
    doc.datasets.push(inline(
        "clients",
        "Клиенты",
        &["client", "banker", "email", "phone", "profile"],
        vec![vec![json!(CLIENT), json!("Иванов Иван Иванович"), json!("i.ivanov@test-pb.ru"), json!("8-999-123-45-67"), json!("Консервативный")]],
        None,
        false,
    ));
    let monthly: Vec<Vec<Json>> = month_ends()
        .into_iter()
        .zip([7.4e6, 5.3e6, 6.2e6, -6.4e6, 6.8e6, 2.4e6])
        .zip([0.021, 0.009, 0.003, -0.018, 0.007, 0.002].into_iter().zip([0.021, 0.030, 0.033, 0.015, 0.022, 0.0234]))
        .map(|((d, inc), (r, c))| vec![json!(d.format("%Y-%m-%d").to_string()), json!(inc), json!(r), json!(c)])
        .collect();
    doc.datasets.push(inline("mon", "Результаты по месяцам", &["date", "income", "ret", "cum_ret"], monthly, Some("date"), false));
    let bench = [
        ("Ваш портфель", 0.0234),
        ("Индекс МосБиржи замещающих", 0.018),
        ("Индекс корп. облигаций", 0.009),
        ("Инфляция", 0.004),
        ("Индекс гособлигаций", -0.018),
        ("Курс USD/RUB", -0.023),
        ("Индекс МосБиржи полн. дох.", -0.041),
    ];
    doc.datasets.push(inline("bench", "Ориентиры", &["name", "ret"], bench.iter().map(|(n, r)| vec![json!(n), json!(r)]).collect(), None, false));
    let vs = [("VS похожие клиенты", 1.2), ("VS индекс МосБиржи", 6.4), ("VS депозит", 0.8), ("VS инфляция", 1.9)];
    doc.datasets.push(inline("vs", "Сравнения", &["metric", "pp"], vs.iter().map(|(n, r)| vec![json!(n), json!(r)]).collect(), None, false));
    let horizons = [("1 месяц", 0.002), ("3 месяца", -0.009), ("6 месяцев", 0.023), ("12 месяцев", 0.084)];
    doc.datasets.push(inline("hor", "Доходность за периоды", &["horizon", "ret"], horizons.iter().map(|(n, r)| vec![json!(n), json!(r)]).collect(), None, false));
    let q = ["2026-02-15", "2026-05-15", "2026-08-15", "2026-11-15", "2027-02-15", "2027-05-15"];
    let flows: Vec<(&str, &str, [f64; 6])> = vec![
        ("Проценты по вкладам", "Накопит. счёт ***23990", [4.8e6, 4.15e6, 4.95e6, 4.95e6, 4.95e6, 4.95e6]),
        ("Проценты по вкладам", "Вклад «Доходный» ***44021", [0.0, 0.0, 6.5e6, 0.0, 0.0, 0.0]),
        ("Проценты по вкладам", "Вклад «Премиум» ***52210", [0.0, 0.0, 0.0, 4.6e6, 0.0, 0.0]),
        ("Дивиденды", "Сбербанк, ао", [0.0, 300e3, 0.0, 0.0, 0.0, 350e3]),
        ("Дивиденды", "Лукойл, ао", [0.0, 0.0, 300e3, 0.0, 250e3, 0.0]),
        ("Дивиденды", "МТС, ао", [350e3, 0.0, 300e3, 0.0, 0.0, 0.0]),
        ("Купоны облигаций", "ОФЗ-26221-ПД", [40e3, 0.0, 40e3, 0.0, 40e3, 0.0]),
        ("Купоны облигаций", "ОФЗ-26238-ПД", [0.0, 40e3, 0.0, 40e3, 0.0, 40e3]),
        ("Возврат капитала", "Вклад «Доходный» · закрытие 05.07.2026", [0.0, 0.0, 60e6, 0.0, 0.0, 0.0]),
        ("Возврат капитала", "Вклад «Премиум» · закрытие 18.10.2026", [0.0, 0.0, 0.0, 42e6, 0.0, 0.0]),
    ];
    let mut cf = Vec::new();
    for (src, inst, vals) in &flows {
        for (i, v) in vals.iter().enumerate() {
            if *v > 0.0 {
                let kind = if i < 2 { "факт" } else { "прогноз" };
                cf.push(vec![json!(q[i]), json!(src), json!(inst), json!(kind), json!(v)]);
            }
        }
    }
    doc.datasets.push(inline("cf", "Денежный поток", &["date", "source", "instrument", "kind", "amount"], cf, Some("date"), false));

    let card_off = json!({"card": false});
    let big = |size: f64| json!({"card": false, "fonts": {"kpi": {"size": size, "weight": 700}, "title": {"size": 7.5, "color": "#857c6f"}}});
    let value = || m("m0", "value", "closing", "Сумма", "number");

    // ---- page 1: cover
    let cover = vec![
        w(json!({"kind": "text", "x": 0, "y": 0, "w": 7, "h": 2, "opts": {"text": "Test Private Banking\nЧастное банковское обслуживание"}, "style": {"fonts": {"body": {"size": 10, "weight": 600}}}})),
        w(json!({"kind": "text", "x": 9, "y": 0, "w": 3, "h": 1, "opts": {"text": "КОНФИДЕНЦИАЛЬНО"}, "style": {"fonts": {"body": {"size": 7.5, "weight": 600, "color": "#857c6f"}}}})),
        w(json!({"kind": "text", "x": 0, "y": 3, "w": 6, "h": 1, "opts": {"text": "ПЕРСОНАЛЬНЫЙ ОТЧЁТ"}, "style": {"fonts": {"body": {"size": 8, "weight": 700, "color": "#b23b3b"}}}})),
        w(json!({"kind": "heading", "x": 0, "y": 4, "w": 9, "h": 5, "locked": true, "opts": {"text": "Отчёт по активам\nи доходности", "subtitle": "за период с {{start}} по {{end}}"}, "style": {"fonts": {"heading": {"size": 38}}}})),
        w(json!({"kind": "divider", "x": 0, "y": 10, "w": 12, "h": 1})),
        w(json!({"kind": "text", "x": 0, "y": 11, "w": 6, "h": 3, "opts": {"text": "ВАШ ПЕРСОНАЛЬНЫЙ БАНКИР\n{{data:Клиенты.banker}}\n{{data:Клиенты.email}} · {{data:Клиенты.phone}}"}, "style": {"fonts": {"body": {"size": 9}}}})),
        w(json!({"kind": "text", "x": 9, "y": 11, "w": 3, "h": 2, "opts": {"text": "ДАТА ФОРМИРОВАНИЯ\n{{today}}"}, "style": {"fonts": {"body": {"size": 9}}}})),
    ];

    // ---- page 2: contents
    let contents = vec![
        w(json!({"kind": "heading", "x": 0, "y": 0, "w": 12, "h": 2, "locked": true, "opts": {"text": "Содержание", "subtitle": "на {{end}}"}, "style": {"fonts": {"heading": {"size": 24}}}})),
        w(json!({"kind": "toc", "title": "", "x": 0, "y": 2, "w": 8, "h": 12, "style": {"card": false}})),
        w(json!({"kind": "tiles", "title": "Отчёт в цифрах", "x": 8, "y": 2, "w": 4, "h": 4, "opts": {"cols": 2, "signColors": false, "highlight": false},
            "binding": bind("pos", &["product"], vec![m("m0", "instrument", "distinct", "Позиций", "number")], json!({"sort": "value_desc", "limit": 4}))})),
    ];

    // ---- page 3: overview
    let overview = vec![
        kpi("Итого активы на {{end}}, ₽", (0, 0, 5, 3), bind("pos", &[], vec![value()], json!({})), json!({"spark": false, "delta": false, "compact": false}), big(40.0)),
        kpi("Доходность", (5, 0, 2, 3), bind("pos", &[], vec![with(m("m0", "income", "closing", "Доходность", "percent"), json!({"divide_by": "start_value"}))], json!({})), json!({"spark": false, "delta": false, "signed": true, "decimals": 2}), big(22.0)),
        kpi("Прибыль", (7, 0, 3, 3), bind("pos", &[], vec![m("m0", "income", "closing", "Прибыль", "currency")], json!({})), json!({"spark": false, "delta": false, "signed": true, "compact": false}), big(22.0)),
        w(json!({"kind": "text", "x": 10, "y": 0, "w": 2, "h": 3, "opts": {"text": "{{data:Клиенты.profile}}\nДенежные средства · 81%\nРоссийский рубль · 93%"}, "style": {"fonts": {"body": {"size": 8.5}}}})),
        w(json!({"kind": "table", "title": "Баланс активов по продуктам", "x": 0, "y": 3, "w": 6, "h": 6, "style": card_off,
            "binding": bind("pos", &["product"], vec![with(m("m0", "value", "closing", "Доля", "percent"), json!({"calc": "share"})), m("m1", "value", "closing", "Сумма, ₽", "number"), with(m("m2", "value", "closing", "%", "percent"), json!({"calc": "share"}))], json!({"sort": "value_desc"})),
            "opts": {"totals": true, "table": {"totalLabel": "Итого активы", "cols": {"m0": {"bar": {"on": true, "only": true, "color": "#9a8a72"}}, "m1": {"bold": true}, "m2": {"color": "#857c6f", "total": "none"}, "product": {"label": "Продукт"}}}}})),
        w(json!({"kind": "table", "title": "Крупнейшие позиции · топ-5", "x": 0, "y": 9, "w": 6, "h": 5, "style": card_off,
            "binding": bind("pos", &["instrument"], vec![with(m("m0", "value", "closing", "#", "number"), json!({"calc": "rank"})), m("m1", "value", "closing", "Сумма, ₽", "currency"), with(m("m2", "value", "closing", "Доля", "percent"), json!({"calc": "share"})), m("m3", "provider", "text", "Источник", "")], json!({"sort": "value_desc", "limit": 5})),
            "opts": {"totals": true, "density": "compact", "table": {"totalLabel": "Итого топ-5", "order": ["m0", "instrument", "m1", "m2"], "cols": {"instrument": {"label": "Позиция", "sub": "m3"}, "m3": {"hidden": true}, "m0": {"color": "#857c6f", "align": "center", "total": "none"}, "m1": {"bold": true, "total": "sum"}, "m2": {"color": "#857c6f", "total": "sum"}}}}})),
        w(json!({"kind": "treemap", "title": "Структура по классам активов", "x": 6, "y": 3, "w": 6, "h": 6, "style": card_off,
            "binding": bind("pos", &["asset_class"], vec![m("m0", "value", "closing", "Сумма", "currency")], json!({})), "opts": {}})),
        w(json!({"kind": "pie", "title": "Структура по валютам", "x": 6, "y": 9, "w": 6, "h": 5, "style": card_off,
            "binding": bind("pos", &["currency"], vec![m("m0", "value", "closing", "Сумма", "currency")], json!({})), "opts": {"donut": true, "legendValues": true, "center": "top", "centerLabel": "рубль", "mono": true}})),
    ];

    // ---- page 4: results
    let ret = |id: &str| with(m(id, "income", "closing", "Доходность", "percent"), json!({"divide_by": "start_value"}));
    let results = vec![
        kpi("Чистый результат с начала года", (0, 0, 4, 3), bind("pos", &[], vec![m("m0", "income", "closing", "Результат", "currency")], json!({})), json!({"spark": false, "delta": false, "signed": true, "compact": false}), big(30.0)),
        kpi("Доход от депозитов", (4, 0, 2, 3), bind("pos", &[], vec![m("m0", "income", "closing", "", "currency")], json!({"filters": [{"field": "product", "op": "=", "value": DEP}]})), json!({"spark": false, "delta": false, "signed": true}), json!({"fonts": {"kpi": {"size": 14}}})),
        kpi("Убыток по инвестициям", (6, 0, 2, 3), bind("pos", &[], vec![m("m0", "income", "closing", "", "currency")], json!({"filters": [{"field": "product", "op": "!=", "value": DEP}]})), json!({"spark": false, "delta": false, "signed": true}), json!({"fonts": {"kpi": {"size": 14}}})),
        kpi("Лучший месяц", (8, 0, 2, 3), bind("mon", &[], vec![m("m0", "income", "max", "", "currency")], json!({})), json!({"spark": false, "delta": false, "signed": true}), json!({"fonts": {"kpi": {"size": 14}}})),
        kpi("Худший месяц", (10, 0, 2, 3), bind("mon", &[], vec![m("m0", "income", "min", "", "currency")], json!({})), json!({"spark": false, "delta": false, "signed": true}), json!({"fonts": {"kpi": {"size": 14}}})),
        w(json!({"kind": "table", "title": "Результат по категориям", "x": 0, "y": 3, "w": 6, "h": 5, "style": card_off,
            "binding": bind("pos", &["product"], vec![m("m0", "income", "closing", "Вклад в результат", "number"), m("m1", "income", "closing", "Результат", "currency"), ret("m2")], json!({"sort": "value_desc"})),
            "opts": {"totals": false, "table": {"cols": {"product": {"label": "Категория"}, "m0": {"bar": {"on": true, "only": true}}, "m1": {"signColor": true, "signed": true, "bold": true}, "m2": {"signColor": true, "signed": true, "decimals": 2}}}}})),
        w(json!({"kind": "waterfall", "title": "Доход по месяцам и накопленный итог", "x": 0, "y": 8, "w": 6, "h": 6, "style": card_off,
            "binding": bind("mon", &["date:month"], vec![m("m0", "income", "sum", "Доход", "currency")], json!({})), "opts": {"totalLabel": "Итог", "units": "millions"}})),
        w(json!({"kind": "tiles", "title": "Доходность по классам активов", "x": 6, "y": 3, "w": 6, "h": 5, "style": card_off,
            "binding": bind("pos", &["asset_class"], vec![m("m0", "income", "closing", "Результат", "currency"), ret("m1")], json!({"sort": "value_desc"})), "opts": {"cols": 2, "signed": true}})),
        w(json!({"kind": "table", "title": "Лидеры роста", "x": 6, "y": 8, "w": 3, "h": 6, "style": card_off,
            "binding": bind("pos", &["instrument"], vec![with(m("m0", "income", "closing", "#", "number"), json!({"calc": "rank"})), m("m1", "income", "closing", "Результат", "currency")], json!({"sort": "value_desc", "limit": 3})),
            "opts": {"totals": false, "table": {"order": ["m0", "instrument", "m1"], "lines": "rows", "cols": {"instrument": {"label": "Инструмент", "wrap": true, "width": 46, "maxLines": 2}, "m0": {"color": "#857c6f"}, "m1": {"signColor": true, "signed": true, "bold": true}}}}})),
        w(json!({"kind": "table", "title": "Аутсайдеры", "x": 9, "y": 8, "w": 3, "h": 6, "style": card_off,
            "binding": bind("pos", &["instrument"], vec![m("m1", "income", "closing", "Результат", "currency")], json!({"sort": "value_asc", "limit": 3})),
            "opts": {"totals": false, "table": {"cols": {"instrument": {"label": "Инструмент", "wrap": true, "width": 58, "maxLines": 2}, "m1": {"signColor": true, "signed": true, "bold": true}}}}})),
    ];

    // ---- page 5: benchmarks
    let bench_page = vec![
        kpi("Доходность с начала года · TWR", (0, 0, 4, 3), bind("mon", &[], vec![m("m0", "cum_ret", "closing", "", "percent")], json!({})), json!({"spark": false, "delta": false, "signed": true, "decimals": 2}), big(34.0)),
        w(json!({"kind": "tiles", "title": "", "x": 4, "y": 0, "w": 8, "h": 3, "style": card_off,
            "binding": bind("vs", &["metric"], vec![m("m0", "pp", "sum", "п.п.", "number")], json!({"window": "all", "sort": "data"})), "opts": {"cols": 4, "signed": true, "suffix": " п.п.", "highlight": false}})),
        w(json!({"kind": "tiles", "title": "", "x": 0, "y": 3, "w": 7, "h": 2, "style": card_off,
            "binding": bind("hor", &["horizon"], vec![m("m0", "ret", "sum", "", "percent")], json!({"window": "all", "sort": "data"})), "opts": {"cols": 4, "signed": true, "highlight": false}})),
        w(json!({"kind": "bar", "title": "Доходность за месяц и итог", "x": 0, "y": 5, "w": 7, "h": 9, "style": card_off,
            "binding": bind("mon", &["date:month"], vec![m("m0", "ret", "sum", "За месяц", "percent"), with(m("m1", "cum_ret", "sum", "итог", "percent"), json!({"chart": "line"}))], json!({})),
            "opts": {"signColors": true, "labels": true}})),
        w(json!({"kind": "bar", "title": "Портфель против ориентиров", "x": 7, "y": 3, "w": 5, "h": 11, "style": card_off,
            "binding": bind("bench", &["name"], vec![m("m0", "ret", "sum", "Доходность", "percent")], json!({"window": "all", "sort": "value_desc"})),
            "opts": {"orientation": "horizontal", "labels": true, "signColors": true, "highlight": "Ваш портфель"}})),
    ];

    // ---- page 6: holdings (continues over pages as needed)
    let holdings = vec![
        w(json!({"kind": "heading", "x": 0, "y": 0, "w": 8, "h": 2, "locked": true, "opts": {"text": "Состав портфеля", "subtitle": "№ 400PX0F · Брокерский счёт"}, "style": {"fonts": {"heading": {"size": 24}}}})),
        kpi("Стоимость счёта", (8, 0, 4, 2), bind("pos", &[], vec![value()], json!({"filters": [{"field": "account", "op": "=", "value": "400PX0F"}]})), json!({"spark": false, "delta": false, "compact": false}), big(24.0)),
        w(json!({"kind": "table", "title": "", "x": 0, "y": 2, "w": 12, "h": 12, "style": card_off,
            "binding": bind("pos", &["asset_class", "instrument"], vec![
                m("m0", "qty", "closing", "Кол-во", "number"),
                with(m("m1", "avg_price", "max", "Ср. цена · FIFO", "number"), json!({})),
                m("m2", "price", "max", "Тек. цена", "number"),
                m("m3", "price_chg", "max", "Изм. цены", "percent"),
                m("m4", "value", "closing", "Стоимость, ₽", "number"),
                m("m5", "dividends", "closing", "Дивиденды / купоны, ₽", "number"),
                m("m6", "income", "closing", "Доход, ₽", "number"),
                ret("m7"),
                with(m("m8", "value", "closing", "Доля", "percent"), json!({"calc": "share"})),
                m("m9", "isin", "text", "ISIN", ""),
            ], json!({"sort": "4:desc", "filters": [{"field": "account", "op": "=", "value": "400PX0F"}]})),
            "opts": {"totals": true, "density": "compact", "table": {
                "subtotals": true, "totalLabel": "Итого по счёту",
                "cols": {
                    "__tree": {"label": "Инструмент", "sub": "m9", "width": 30},
                    "m9": {"hidden": true},
                    "m0": {"total": "none", "zero": "dash"}, "m1": {"total": "none", "decimals": 2}, "m2": {"total": "none", "decimals": 2},
                    "m3": {"signColor": true, "signed": true, "total": "none"},
                    "m4": {"bold": true},
                    "m5": {"signed": true, "color": "#1f7a5c", "zero": "dash"},
                    "m6": {"signColor": true, "signed": true},
                    "m7": {"signColor": true, "signed": true, "rules": [
                        {"op": "<", "value": "-0.19", "fill": "#f6e3df", "row": true, "accent": "#b23b3b"},
                        {"op": ">", "value": "0.15", "fill": "#e2efe8", "row": true, "accent": "#1f7a5c"}
                    ]},
                    "m8": {"color": "#857c6f"}
                }}}})),
    ];

    // ---- page 7: cash flow
    let cash = vec![
        w(json!({"kind": "heading", "x": 0, "y": 0, "w": 6, "h": 2, "locked": true, "opts": {"text": "Поступления по кварталам", "subtitle": "12 месяцев вперёд · факт и прогноз"}, "style": {"fonts": {"heading": {"size": 24}}}})),
        kpi("Получено за период", (6, 0, 2, 2), bind("cf", &[], vec![m("m0", "amount", "sum", "", "compact")], json!({"window": "all", "filters": [{"field": "kind", "op": "=", "value": "факт"}]})), json!({"spark": false, "delta": false}), json!({"fonts": {"kpi": {"size": 16}}})),
        kpi("Регулярный доход · прогноз", (8, 0, 2, 2), bind("cf", &[], vec![m("m0", "amount", "sum", "", "compact")], json!({"window": "all", "filters": [{"field": "kind", "op": "=", "value": "прогноз"}, {"field": "source", "op": "!=", "value": "Возврат капитала"}]})), json!({"spark": false, "delta": false}), json!({"fonts": {"kpi": {"size": 16}}})),
        kpi("Возврат капитала впереди", (10, 0, 2, 2), bind("cf", &[], vec![m("m0", "amount", "sum", "", "compact")], json!({"window": "all", "filters": [{"field": "source", "op": "=", "value": "Возврат капитала"}]})), json!({"spark": false, "delta": false}), json!({"fonts": {"kpi": {"size": 16}}})),
        w(json!({"kind": "table", "title": "Денежный поток по источникам", "x": 0, "y": 2, "w": 12, "h": 12, "style": card_off,
            "binding": bind("cf", &["source", "instrument"], vec![m("m0", "amount", "sum", "", "compact")], json!({"window": "all", "pivot": "date:quarter", "sort": "dim_asc"})),
            "opts": {"totals": true, "table": {"subtotals": true, "totalLabel": "Итого приток", "cols": {"__tree": {"label": "Источник", "width": 30}, "m0": {"scale": {"on": true, "from": "#f4efe6", "to": "#c9b48a"}}}}}})),
    ];

    // ---- page 8: legal
    let legal_a = "Информация, изложенная в настоящем отчёте, подготовлена специалистами Test Private Banking с единственной целью — предоставить сводную информацию общего характера о вложениях Ваших средств в компаниях группы и у её партнёров.\n\nНастоящий отчёт не предназначен для обоснования инвестиционного решения, не является официальным источником отчётности и не должен рассматриваться как финансовая рекомендация.\n\nОтчёт подготовлен на основе материалов, полученных от компаний группы и партнёров. Test Private Banking не гарантирует их полноту и достоверность.";
    let legal_b = "Настоящий отчёт не является предложением о продаже ценных бумаг или подписке на них, а также рекламой ценных бумаг.\n\nОтчёт и его содержание являются конфиденциальной информацией; без согласия Test Private Banking его распространение и раскрытие третьим лицам запрещено.\n\nДля получения официальной отчётности обратитесь в компанию группы или к партнёру, в которых обслуживаются Ваши активы.";
    let legal = vec![
        w(json!({"kind": "heading", "x": 0, "y": 0, "w": 12, "h": 2, "locked": true, "opts": {"text": "Юридическая информация", "subtitle": "Конфиденциально · только для адресата"}, "style": {"fonts": {"heading": {"size": 24}}}})),
        w(json!({"kind": "text", "x": 0, "y": 2, "w": 6, "h": 10, "opts": {"text": legal_a}, "style": {"fonts": {"body": {"size": 8.5}}}})),
        w(json!({"kind": "text", "x": 6, "y": 2, "w": 6, "h": 7, "opts": {"text": legal_b}, "style": {"fonts": {"body": {"size": 8.5}}}})),
        w(json!({"kind": "text", "title": "Реквизиты и лицензии", "x": 6, "y": 10, "w": 6, "h": 4, "opts": {"text": "Тест Банк (ПАО) · Генеральная лицензия на осуществление банковских операций № 1481\nУК Капитал (АО) · Лицензия на управление инвестиционными фондами"}, "style": {"card": true, "fonts": {"body": {"size": 8}}}})),
    ];

    doc.pages.clear();
    let sections = [
        ("Обложка", "", true, cover),
        ("Содержание", "", false, contents),
        ("Обзор портфеля", "Обзор портфеля", false, overview),
        ("Результаты", "Ключевые показатели", false, results),
        ("Сравнение и ориентиры", "Ключевые показатели", false, bench_page),
        ("Брокерский счёт 400PX0F", "Детальная информация", false, holdings),
        ("Денежный поток", "Прогноз", false, cash),
        ("Юридическая информация", "Юридическая", false, legal),
    ];
    for (title, section, plain, widgets) in sections {
        let id = doc.new_id("p");
        let widgets = widgets.into_iter().map(|x| doc.instantiate(x)).collect();
        doc.pages.push(Page { id, title: title.into(), widgets, auto: false, section: section.into(), plain, ..Default::default() });
    }
    doc.arrange = crate::layout::Options { auto: true, fill: false };
    doc.normalize();
    // monthly values and their running total: two axes, the total labelled at its last value
    doc.split_series_axes();
    doc
}
