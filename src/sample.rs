//! A complete sample template: "Sales Performance Review", with generated
//! data so it works without any files or databases.

use crate::model::*;
use crate::period::{add_months, month_start};
use chrono::{Datelike, Local};
use serde_json::{json, Value as Json};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
}

fn sales_rows() -> (Vec<String>, Vec<Vec<Json>>, chrono::NaiveDate) {
    let last = add_months(month_start(Local::now().date_naive()), -1);
    let first = add_months(last, -32);
    let regions = [("North", 1.25), ("South", 0.9), ("East", 1.05), ("West", 0.75)];
    let products: [(&str, f64, f64); 5] = [("Aurora", 420.0, 0.42), ("Borealis", 260.0, 0.35), ("Cirrus", 180.0, 0.51), ("Nimbus", 95.0, 0.28), ("Stratus", 640.0, 0.46)];
    let channels = [("Direct", 1.0, 0.0), ("Partner", 0.7, -0.06), ("Online", 0.85, 0.04)];
    let mut rng = Lcg(42);
    let mut rows = Vec::new();
    let mut d = first;
    let mut i = 0.0;
    while d <= last {
        let season = 1.0 + 0.12 * ((d.month() as f64 - 3.0) / 12.0 * std::f64::consts::TAU).sin() + if d.month() == 12 { 0.18 } else { 0.0 };
        let growth = 1.0 + 0.017 * i;
        for (r, rw) in regions {
            for (p, price, margin) in products {
                for (c, cw, cm) in channels {
                    let base = 38.0 * rw * cw * season * growth * (0.8 + 0.4 * rng.next());
                    let units = (base * (180.0 / price).sqrt()).round().max(1.0);
                    let revenue = (units * price * (0.95 + 0.1 * rng.next())).round();
                    let gm = (margin + cm + 0.04 * (rng.next() - 0.5)).clamp(0.05, 0.8);
                    let gross = (revenue * gm).round();
                    let customers = (units / (2.0 + 3.0 * rng.next())).round().max(1.0);
                    rows.push(vec![
                        json!(d.format("%Y-%m-%d").to_string()),
                        json!(r),
                        json!(p),
                        json!(c),
                        json!(revenue),
                        json!(revenue - gross),
                        json!(gross),
                        json!(units),
                        json!(customers),
                    ]);
                }
            }
        }
        d = add_months(d, 1);
        i += 1.0;
    }
    let cols = ["date", "region", "product", "channel", "revenue", "cost", "gross_profit", "units", "customers"];
    (cols.iter().map(|s| s.to_string()).collect(), rows, last)
}

fn m(field: &str, agg: &str, label: &str, format: &str) -> Measure {
    Measure { field: field.into(), agg: agg.into(), label: label.into(), format: format.into(), ..Default::default() }
}

fn ratio(field: &str, by: &str, label: &str) -> Measure {
    Measure { field: field.into(), agg: "sum".into(), label: label.into(), format: "percent".into(), divide_by: Some(by.into()), ..Default::default() }
}

fn calc(mut m: Measure, c: &str) -> Measure {
    m.calc = c.into();
    m
}

fn region_filter() -> Vec<Filter> {
    vec![Filter { field: "region".into(), op: "=".into(), value: "{{region}}".into() }]
}

fn bind(dims: &[&str], measures: Vec<Measure>, window: &str) -> Option<Binding> {
    Some(Binding {
        dataset: "sales".into(),
        dims: dims.iter().map(|s| s.to_string()).collect(),
        measures,
        window: window.into(),
        filters: region_filter(),
        ..Default::default()
    })
}

fn widget(kind: &str, title: &str, (x, y, w, h): (u32, u32, u32, u32), binding: Option<Binding>, opts: Json) -> Widget {
    Widget { kind: kind.into(), title: title.into(), x, y, w, h, binding, opts, ..Default::default() }
}

pub fn template() -> Doc {
    let (columns, rows, last) = sales_rows();
    let mut doc = Doc::default();
    doc.meta.title = "Sales Performance Review".into();
    doc.meta.subtitle = "Monthly management report".into();
    doc.meta.organization = "Northwind Analytics".into();
    doc.meta.description = "Revenue, margin and volume by region, product and channel, with commentary.".into();
    doc.params.date = last;
    doc.params.grain = "month".into();
    doc.param_defs = vec![ParamDef {
        name: "region".into(),
        label: "Region".into(),
        options: vec!["North".into(), "South".into(), "East".into(), "West".into()],
        default: "All".into(),
        ..Default::default()
    }];
    doc.params.values.insert("region".into(), "All".into());
    doc.datasets.push(Dataset {
        id: "sales".into(),
        name: "Sales".into(),
        source: Source::Inline { columns, rows },
        date_field: Some("date".into()),
        fields: Vec::new(),
        snapshot: false,
        ..Default::default()
    });

    let kpi_revenue = widget("kpi", "Revenue", (0, 2, 3, 2), bind(&[], vec![m("revenue", "sum", "Revenue", "currency")], "period"), json!({}));
    let kpi_margin = widget(
        "kpi",
        "Gross margin",
        (3, 2, 3, 2),
        bind(&[], vec![ratio("gross_profit", "revenue", "Gross margin")], "period"),
        json!({}),
    );
    let kpi_units = widget("kpi", "Units sold", (6, 2, 3, 2), bind(&[], vec![m("units", "sum", "Units", "number")], "period"), json!({}));
    let kpi_customers = widget(
        "kpi",
        "Active customers",
        (9, 2, 3, 2),
        bind(&[], vec![m("customers", "sum", "Customers", "number")], "period"),
        json!({}),
    );
    let trend = widget(
        "line",
        "Revenue and gross profit",
        (0, 4, 8, 5),
        bind(
            &["date:month"],
            vec![m("revenue", "sum", "Revenue", "currency"), m("gross_profit", "sum", "Gross profit", "currency")],
            "trailing12",
        ),
        json!({}),
    );
    let by_region = widget(
        "bar",
        "Revenue by region",
        (8, 4, 4, 5),
        Some(Binding {
            dataset: "sales".into(),
            dims: vec!["region".into()],
            measures: vec![m("revenue", "sum", "Revenue", "currency")],
            window: "period".into(),
            ..Default::default()
        }),
        json!({"orientation": "horizontal", "labels": true}),
    );
    let mut heading = widget(
        "heading",
        "",
        (0, 0, 12, 2),
        None,
        json!({"text": "{{title}}", "subtitle": "{{period}} · Region: {{region}} · compared with {{compare}}"}),
    );
    heading.locked = true;
    let comment = widget(
        "comment",
        "",
        (0, 9, 12, 3),
        None,
        json!({
            "author": "Finance team",
            "text": "Revenue grew against the comparison period, led by the North region and the Stratus product line.\n- Margin held steady as partner discounts were offset by a richer online mix.\n- Watch the West region: volumes are flat for the third month in a row."
        }),
    );

    let mut h2 = widget("heading", "", (0, 0, 12, 1), None, json!({"text": "Product & channel detail"}));
    h2.style = json!({"size": 15});
    h2.locked = true;
    let product_table = widget(
        "table",
        "Product performance",
        (0, 1, 7, 6),
        bind(
            &["product"],
            vec![
                m("revenue", "sum", "Revenue", "currency"),
                calc(m("revenue", "sum", "vs prior", "percent"), "pct"),
                m("gross_profit", "sum", "Gross profit", "currency"),
                ratio("gross_profit", "revenue", "Margin"),
                m("units", "sum", "Units", "number"),
            ],
            "period",
        ),
        json!({"totals": true, "table": {
            "zebra": true,
            "cols": {
                "m0": {"bar": {"on": true}},
                "m1": {"arrows": true, "signColor": true, "decimals": 1},
                "m3": {"rules": [{"op": "<", "value": "0.3", "color": "#b42318", "bold": true}]}
            }
        }}),
    );
    let channel_mix = widget(
        "pie",
        "Channel mix",
        (7, 1, 5, 6),
        bind(&["channel"], vec![m("revenue", "sum", "Revenue", "currency")], "period"),
        json!({"donut": true}),
    );
    let monthly_region = widget(
        "bar",
        "Monthly revenue by region",
        (0, 7, 12, 5),
        Some(Binding {
            dataset: "sales".into(),
            dims: vec!["date:month".into(), "region".into()],
            measures: vec![m("revenue", "sum", "Revenue", "currency")],
            window: "trailing6".into(),
            filters: region_filter(),
            ..Default::default()
        }),
        json!({"stacked": true}),
    );

    let p1 = vec![heading, kpi_revenue.clone(), kpi_margin.clone(), kpi_units.clone(), kpi_customers.clone(), trend.clone(), by_region.clone(), comment.clone()];
    let p2 = vec![h2, product_table.clone(), channel_mix.clone(), monthly_region.clone()];
    doc.pages.clear();
    for (title, ws) in [("Executive summary", p1), ("Product & channel detail", p2)] {
        let id = doc.new_id("p");
        let widgets = ws.into_iter().map(|w| doc.instantiate(w)).collect();
        doc.pages.push(Page { id, title: title.into(), widgets, auto: false, section: String::new(), plain: false, ..Default::default() });
    }

    // the widgets end users may add
    let by_product = widget(
        "bar",
        "Revenue by product",
        (0, 0, 6, 4),
        bind(&["product"], vec![m("revenue", "sum", "Revenue", "currency")], "period"),
        json!({"orientation": "horizontal", "labels": true}),
    );
    let region_table = widget(
        "table",
        "Region performance",
        (0, 0, 6, 4),
        Some(Binding {
            dataset: "sales".into(),
            dims: vec!["region".into()],
            measures: vec![
                m("revenue", "sum", "Revenue", "currency"),
                ratio("gross_profit", "revenue", "Margin"),
                m("customers", "sum", "Customers", "number"),
            ],
            window: "period".into(),
            ..Default::default()
        }),
        json!({"totals": true}),
    );
    let rev_per_customer = widget(
        "kpi",
        "Revenue per customer",
        (0, 0, 3, 2),
        bind(&[], vec![ratio("revenue", "customers", "Revenue per customer").with_format("currency")], "period"),
        json!({}),
    );
    let ytd = widget("kpi", "Revenue YTD", (0, 0, 3, 2), bind(&[], vec![m("revenue", "sum", "Revenue YTD", "currency")], "ytd"), json!({}));
    let area = widget(
        "area",
        "Units trend",
        (0, 0, 6, 4),
        bind(&["date:month"], vec![m("units", "sum", "Units", "number")], "trailing12"),
        json!({}),
    );
    let orders = widget(
        "table",
        "Order lines",
        (0, 0, 8, 5),
        Some(Binding {
            dataset: "sales".into(),
            mode: "rows".into(),
            columns: vec!["date".into(), "region".into(), "product".into(), "channel".into(), "units".into(), "revenue".into()],
            window: "period".into(),
            sort: "5:desc".into(),
            filters: region_filter(),
            ..Default::default()
        }),
        json!({"totals": true, "table": {
            "header": {"upper": true},
            "cols": {
                "channel": {"pill": {"on": true, "rules": [
                    {"match": "Direct", "fill": "#e1ecfb", "color": "#1c5cab"},
                    {"match": "Partner", "fill": "#ece9fb", "color": "#4a3aa7"},
                    {"match": "Online", "fill": "#e3f4e7", "color": "#0f6b2c"}
                ]}},
                "revenue": {"format": "currency"}
            }
        }}),
    );
    let note = widget("comment", "", (0, 0, 6, 2), None, json!({"text": ""}));
    let text = widget("text", "", (0, 0, 6, 2), None, json!({"text": "Notes and context for the reader."}));
    let catalog: Vec<(&str, &str, &str, Widget)> = vec![
        ("KPIs", "Revenue", "Total revenue for the period with change and 12-month trend.", kpi_revenue),
        ("KPIs", "Gross margin", "Gross profit as a share of revenue.", kpi_margin),
        ("KPIs", "Units sold", "Units sold in the period.", kpi_units),
        ("KPIs", "Active customers", "Customers buying in the period.", kpi_customers),
        ("KPIs", "Revenue per customer", "Average revenue per active customer.", rev_per_customer),
        ("KPIs", "Revenue YTD", "Year-to-date revenue.", ytd),
        ("Charts", "Revenue trend", "Revenue and gross profit over the last 12 months.", trend),
        ("Charts", "Revenue by region", "Ranked regions for the period.", by_region),
        ("Charts", "Revenue by product", "Ranked products for the period.", by_product),
        ("Charts", "Channel mix", "Share of revenue by sales channel.", channel_mix),
        ("Charts", "Monthly revenue by region", "Stacked monthly revenue, last 6 months.", monthly_region),
        ("Charts", "Units trend", "Units sold over the last 12 months.", area),
        ("Tables", "Product performance", "Revenue, profit, margin and units per product.", product_table),
        ("Tables", "Region performance", "Revenue, margin and customers per region.", region_table),
        ("Tables", "Order lines", "Largest order lines for the period, with channel badges.", orders),
        ("Text", "Comment", "A commentary note with your name.", note),
        ("Text", "Text block", "Plain text without a card.", text),
    ];
    for (cat, name, desc, w) in catalog {
        let id = doc.new_id("c");
        let mut w = w;
        w.id = String::new();
        w.locked = false;
        w.y = 0;
        w.x = 0;
        doc.catalog.push(CatalogItem { id, name: name.into(), description: desc.into(), category: cat.into(), widget: w, page: String::new() });
    }
    doc.normalize();
    doc
}

trait WithFormat {
    fn with_format(self, f: &str) -> Self;
}

impl WithFormat for Measure {
    fn with_format(mut self, f: &str) -> Self {
        self.format = f.into();
        self
    }
}

/// A blank template for designers to start from.
pub fn blank() -> Doc {
    let mut doc = Doc::default();
    doc.meta.title = "New report template".into();
    doc.params.date = add_months(month_start(Local::now().date_naive()), -1);
    let mut heading = widget("heading", "", (0, 0, 12, 2), None, json!({"text": "{{title}}", "subtitle": "{{period}}"}));
    heading.locked = true;
    let heading = doc.instantiate(heading);
    doc.pages[0].widgets.push(heading);
    doc
}
