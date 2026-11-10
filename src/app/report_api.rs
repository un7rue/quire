//! Reports other systems ask for (the report API, backlog/report-api.md): a published report for
//! one of the bank's clients, given by the bank's client id, over a period, with the report's other
//! parameters. It is made apart from the open documents, with the caller's access, as a batch
//! report is; the pages are then drawn without a browser (src/render_js.rs).
// the server's endpoints call it (backlog/report-api.md, step 4); until then only the tests do
#![cfg_attr(not(test), allow(dead_code))]

use super::*;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

/// The report made when a request names none.
pub const DEFAULT_TEMPLATE: &str = "uhnwi";

/// What a caller asks for.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// A published report's id ("uhnwi"); the default report when left out.
    #[serde(default)]
    pub template: Option<String>,
    /// The bank's id of the client (clients.id).
    pub client_id: String,
    pub period: PeriodAsked,
    /// The report's other parameters by name: a text, a number, or a list for one that takes several.
    #[serde(default)]
    pub params: BTreeMap<String, Json>,
}

/// The period: two dates, or a named one counted back from today.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PeriodAsked {
    Dates { start: String, end: String },
    Preset { preset: String },
}

/// The named periods: this year so far, and the last whole month, quarter or year.
pub const PRESETS: [&str; 4] = ["ytd", "last_month", "last_quarter", "last_year"];

/// Why a request was turned down, with the HTTP status it answers with.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub status: u16,
    pub message: String,
}

fn not_found(m: impl Into<String>) -> Refusal {
    Refusal { status: 404, message: m.into() }
}
fn forbidden(m: impl Into<String>) -> Refusal {
    Refusal { status: 403, message: m.into() }
}
fn invalid(m: impl Into<String>) -> Refusal {
    Refusal { status: 422, message: m.into() }
}

/// A report made for a request: its state (what the pages are drawn from) and the values it was
/// made with, defaults included.
pub(super) struct Made {
    pub state: Json,
    pub used: Json,
}

/// The first and last day of the period asked for; named periods count back from `today`.
fn dates(p: &PeriodAsked, today: NaiveDate) -> Result<(NaiveDate, NaiveDate), Refusal> {
    let (s, e) = match p {
        PeriodAsked::Dates { start, end } => {
            let d = |s: &str, what: &str| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|_| invalid(format!("The period's {what} “{s}” isn't a date (yyyy-mm-dd).")));
            (d(start, "start")?, d(end, "end")?)
        }
        PeriodAsked::Preset { preset } => {
            let month = period::month_start(today);
            let quarter = period::ymd(today.year(), today.month0() / 3 * 3 + 1, 1);
            match preset.as_str() {
                "ytd" => (period::ymd(today.year(), 1, 1), today),
                "last_month" => (period::add_months(month, -1), month - chrono::Duration::days(1)),
                "last_quarter" => (period::add_months(quarter, -3), quarter - chrono::Duration::days(1)),
                "last_year" => (period::ymd(today.year() - 1, 1, 1), period::ymd(today.year() - 1, 12, 31)),
                p => return Err(invalid(format!("Unknown period “{p}”: use one of {}, or start and end dates.", PRESETS.join(", ")))),
            }
        }
    };
    if s > e {
        return Err(invalid(format!("The period starts ({s}) after it ends ({e}).")));
    }
    Ok((s, e))
}

/// A parameter's value as the report keeps it: several values joined by U+001F.
fn value_of(def: &ParamDef, v: &Json) -> Result<Vec<String>, Refusal> {
    let one = |v: &Json| match v {
        Json::String(s) => Ok(s.trim().to_string()),
        Json::Number(n) => Ok(n.to_string()),
        Json::Bool(b) => Ok(b.to_string()),
        _ => Err(invalid(format!("“{}” takes a text or a number.", def.name))),
    };
    let vals = match v {
        Json::Array(a) if def.multi => a.iter().map(one).collect::<Result<Vec<_>, _>>()?,
        Json::Array(_) => return Err(invalid(format!("“{}” takes one value.", def.name))),
        v => vec![one(v)?],
    };
    if vals.is_empty() || vals.iter().any(|s| s.is_empty()) {
        return Err(invalid(format!("“{}” is empty.", def.name)));
    }
    for s in &vals {
        match def.kind.as_str() {
            "number" if s.parse::<f64>().is_err() => return Err(invalid(format!("“{}” takes a number, not “{s}”.", def.name))),
            "date" if NaiveDate::parse_from_str(s, "%Y-%m-%d").is_err() => return Err(invalid(format!("“{}” takes a date (yyyy-mm-dd), not “{s}”.", def.name))),
            _ => {}
        }
        if def.source.is_none() && !def.options.is_empty() && s != "All" && !def.options.contains(s) {
            return Err(invalid(format!("“{s}” isn't one of the choices for “{}”: {}.", def.name, def.options.join(", "))));
        }
    }
    Ok(vals)
}

/// Makes the report a request asks for, as the signed-in person (or API account) of `app`;
/// `today` dates the named periods.
pub(super) fn make(app: &App, req: &Request, today: NaiveDate) -> Result<Made, Refusal> {
    gate(app, &["user", "design"]).map_err(forbidden)?;
    let session = app.session.as_ref().ok_or_else(|| forbidden("Sign in first."))?;
    // the report
    let id = req.template.as_deref().map(str::trim).filter(|t| !t.is_empty()).unwrap_or(DEFAULT_TEMPLATE);
    let id = id.strip_prefix("pub:").unwrap_or(id);
    let st = app.store();
    let p = st.reports.iter().find(|r| r.id == id).ok_or_else(|| not_found(format!("No published report “{id}”.")))?;
    // an API account asks for the reports Admin gave it; a person for the ones published to them
    let may = match session.login.strip_prefix("api:") {
        Some(account) => p.published && st.api.clients.iter().any(|c| c.id == account && c.may_use(id)),
        None => st.may_use(p, session),
    };
    if !may {
        return Err(forbidden(format!("“{}” isn't one of your reports.", p.title)));
    }
    let rules = st.api.reports.get(id).cloned().unwrap_or_default();
    if rules.off {
        return Err(forbidden(format!("“{}” isn't offered over the API.", p.title)));
    }
    // the client, by the bank's id
    let client_id = req.client_id.trim();
    if client_id.is_empty() {
        return Err(invalid("client_id is empty."));
    }
    let opts = auth::options(&st).map_err(|e| Refusal { status: 500, message: e })?;
    let client = opts["clients"].as_array().and_then(|a| a.iter().find(|c| c["id"] == client_id)).and_then(|c| c["name"].as_str()).map(String::from)
        .ok_or_else(|| not_found(format!("No client with id “{client_id}”.")))?;
    if let Some(allowed) = &app.clients {
        if !client_ok(allowed, client_id) {
            return Err(forbidden(format!("Client “{client_id}” is outside your access.")));
        }
    }
    let (start, end) = dates(&req.period, today)?;

    let mut d = for_client(report_doc(app, id).map_err(|e| Refusal { status: 500, message: e })?, Some(&client));
    d.params.grain = "custom".into();
    d.params.start = Some(start);
    d.params.date = end;
    // the other parameters: those Admin lets the API set, checked against the report's own
    let settable = |p: &ParamDef| rules.params.get(&p.name).is_some_and(|r| r.accept) && !auth::is_client_param(p);
    let mut asked: Vec<(ParamDef, Vec<String>)> = Vec::new();
    for (name, v) in &req.params {
        let takes = || {
            let names: Vec<String> = d.param_defs.iter().filter(|p| settable(p)).map(|p| p.name.clone()).collect();
            if names.is_empty() { "none (Admin → API sets which)".to_string() } else { names.join(", ") }
        };
        let def = d.param_defs.iter().find(|p| &p.name == name).cloned()
            .ok_or_else(|| invalid(format!("“{}” has no parameter “{name}”; over the API it takes: {}.", p.title, takes())))?;
        if auth::is_client_param(&def) {
            return Err(invalid(format!("“{name}” is the client: give it as client_id.")));
        }
        if !settable(&def) {
            return Err(invalid(format!("“{name}” can't be set over the API; it takes: {}.", takes())));
        }
        let vals = value_of(&def, v)?;
        d.params.values.insert(name.clone(), vals.join("\u{1f}"));
        asked.push((def, vals));
    }
    // Admin's defaults for API requests, for what the request leaves out
    let mut defaults: Vec<(ParamDef, Vec<String>)> = Vec::new();
    for (name, r) in rules.params.iter().filter(|(n, r)| !r.default.is_null() && !req.params.contains_key(*n)) {
        let Some(def) = d.param_defs.iter().find(|p| &p.name == name).cloned() else { continue };
        let vals = value_of(&def, &r.default).map_err(|e| Refusal { status: 500, message: format!("The API's default for {}", e.message) })?;
        d.params.values.insert(name.clone(), vals.join("\u{1f}"));
        defaults.push((def, vals));
    }
    let mut sub = load_apart(app, d, &client).map_err(forbidden)?;

    // values read from the data: one of its choices, and within the caller's products
    let products: Vec<String> = session.products().split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    if asked.iter().chain(&defaults).any(|(def, _)| def.source.is_some()) {
        let base = sub.base_dir();
        let choices = {
            let App { doc, engine, py, .. } = &mut sub;
            doc.as_ref().map(|doc| engine.param_options(doc, base.as_deref(), py)).unwrap_or(Json::Null)
        };
        for (k, (def, vals)) in asked.iter().chain(&defaults).enumerate().filter(|(_, (def, _))| def.source.is_some()) {
            let product = def.source.as_ref().is_some_and(|s| s.field == "src_cd") || def.field == "src_cd";
            for v in vals.iter().filter(|v| v.as_str() != "All") {
                // (a default outside the caller's products shows nothing of them: access filters the data)
                if k < asked.len() && product && !products.is_empty() && !products.contains(v) {
                    return Err(forbidden(format!("Product “{v}” is outside your access.")));
                }
                let known = choices[&def.name].as_array().map(|a| a.iter().any(|c| c == v.as_str()));
                if known == Some(false) {
                    return Err(invalid(format!("“{v}” isn't one of the choices for “{}”.", def.name)));
                }
            }
        }
    }

    let state = sub.resp();
    let doc = sub.doc.as_ref().ok_or_else(|| Refusal { status: 500, message: "The report didn't load.".into() })?;
    let params: serde_json::Map<String, Json> = doc.param_defs.iter().filter(|p| !auth::is_client_param(p)).map(|p| {
        let v = doc.params.values.get(&p.name).cloned().unwrap_or_else(|| p.default.clone());
        let v = if p.multi { json!(v.split('\u{1f}').filter(|s| !s.is_empty()).collect::<Vec<_>>()) } else { json!(v) };
        (p.name.clone(), v)
    }).collect();
    let used = json!({
        "template": id,
        "title": doc.meta.title,
        "client_id": client_id,
        "client": client,
        "period": {"start": start.to_string(), "end": end.to_string(), "label": state["period"]["label"]},
        "params": params,
    });
    Ok(Made { state, used })
}

/// A report as a file: the PDF, its name, its page count and the layout issues that need a look
/// (content cut off or running past the page).
#[cfg(feature = "render")]
pub(super) struct Drawn {
    pub pdf: Vec<u8>,
    pub name: String,
    pub pages: usize,
    pub issues: usize,
}

/// Draws a made report's pages with the UI's drawing code in `ui` and writes them as a PDF.
#[cfg(feature = "render")]
pub(super) fn draw(ui: &Path, made: &Made) -> Result<Drawn, Refusal> {
    let fail = |message: String| Refusal { status: 500, message };
    let st = &made.state;
    let svgs = crate::render_js::pages(ui, st).map_err(fail)?;
    let shown: Vec<&Json> = st["pages"].as_array().map(|a| a.iter().filter(|p| p["hidden"] != true).collect()).unwrap_or_default();
    let issues = shown.iter().map(|p| p["issues"].as_array().map(|a| a.iter().filter(|x| x["severity"] != "info").count()).unwrap_or(0)).sum();
    let title = made.used["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Report");
    let name = format!("{} — {title} — {}", made.used["client"].as_str().unwrap_or(""), made.used["period"]["label"].as_str().unwrap_or(""));
    let (w, h) = (st["page"]["w"].as_f64().unwrap_or(842.0), st["page"]["h"].as_f64().unwrap_or(595.0));
    let pdf = export::pdf(&svgs, w, h, &name).map_err(fail)?;
    Ok(Drawn { pdf, name, pages: svgs.len(), issues })
}

#[cfg(test)]
mod tests {
    use super::super::tests::app_with;
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// The accounts store in a folder of its own, with `change` made to it; the API takes the
    /// products for the UHNWI report.
    fn store_with(change: impl FnOnce(&mut auth::Store)) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("quire_api_{}_{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut st = auth::seed();
        let mut r = crate::api_accounts::ApiReport::default();
        r.params.insert("product".into(), crate::api_accounts::ApiParam { accept: true, default: Json::Null });
        st.api.reports.insert("uhnwi".into(), r);
        change(&mut st);
        let path = dir.join("users.json");
        auth::save(&path, &st).unwrap();
        path
    }

    /// A banker signed in, with a report open (it stays as it is).
    fn banker() -> App {
        banker_with(|_| {})
    }

    fn banker_with(change: impl FnOnce(&mut auth::Store)) -> App {
        let mut doc = uhnwi_fixed();
        to_report(&mut doc, None);
        let mut app = app_with(doc);
        app.store_path = Some(store_with(change));
        app.start_session(auth::sign_in(&app.store(), "user", auth::DEMO_PASSWORD).unwrap()).unwrap();
        app
    }

    fn ask(v: Json) -> Request {
        serde_json::from_value(v).unwrap()
    }

    /// The bank's client id is the client's; one of the banker's own clients.
    fn own_client(app: &App) -> (String, String) {
        app.clients.as_ref().unwrap().first().cloned().unwrap()
    }

    #[test]
    fn named_periods_count_back_from_today() {
        let t = d("2026-11-10");
        let p = |s: &str| dates(&PeriodAsked::Preset { preset: s.into() }, t).unwrap();
        assert_eq!(p("ytd"), (d("2026-01-01"), t));
        assert_eq!(p("last_month"), (d("2026-10-01"), d("2026-10-31")));
        assert_eq!(p("last_quarter"), (d("2026-07-01"), d("2026-09-30")));
        assert_eq!(p("last_year"), (d("2025-01-01"), d("2025-12-31")));
        assert_eq!(dates(&PeriodAsked::Preset { preset: "last_quarter".into() }, d("2026-02-15")).unwrap(), (d("2025-10-01"), d("2025-12-31")));
        assert_eq!(dates(&PeriodAsked::Preset { preset: "week".into() }, t).unwrap_err().status, 422);
        let dd = |a: &str, b: &str| dates(&PeriodAsked::Dates { start: a.into(), end: b.into() }, t);
        assert_eq!(dd("2026-07-01", "2026-09-30").unwrap(), (d("2026-07-01"), d("2026-09-30")));
        assert_eq!(dd("2026-09-30", "2026-07-01").unwrap_err().status, 422);
        assert_eq!(dd("30.09.2026", "2026-10-01").unwrap_err().status, 422);
    }

    /// The report for a client given by the bank's id, over the period asked; the template defaults
    /// to UHNWI; the values used come back, defaults included; the open document is untouched.
    #[test]
    fn makes_the_report_asked_for() {
        if dbconn::demo_path().is_none() { return }
        let app = banker();
        let before = serde_json::to_string(app.doc.as_ref().unwrap()).unwrap();
        let (id, name) = own_client(&app);
        let m = make(&app, &ask(json!({"client_id": id, "period": {"start": "2026-07-01", "end": "2026-09-30"}})), d("2026-11-10")).unwrap();
        assert_eq!(m.state["doc"]["params"]["values"]["client"], name.as_str());
        assert_eq!(m.state["doc"]["params"]["start"], "2026-07-01");
        assert_eq!(m.state["doc"]["params"]["date"], "2026-09-30");
        assert!(m.state["pages"].as_array().unwrap().len() > 10);
        assert_eq!(m.used["template"], "uhnwi");
        assert_eq!(m.used["client_id"], id.as_str());
        assert_eq!(m.used["client"], name.as_str());
        assert_eq!(m.used["period"]["start"], "2026-07-01");
        assert_eq!(m.used["params"]["product"], json!(["All"]), "{}", m.used);
        assert!(m.used["params"].get("client").is_none());
        assert_eq!(serde_json::to_string(app.doc.as_ref().unwrap()).unwrap(), before, "the open document is untouched");
        // a named period, and products
        let m = make(&app, &ask(json!({"template": "pub:uhnwi", "client_id": id, "period": {"preset": "last_quarter"}, "params": {"product": ["BROK"]}})), d("2026-11-10")).unwrap();
        assert_eq!(m.used["period"]["end"], "2026-09-30");
        assert_eq!(m.used["params"]["product"], json!(["BROK"]));
        assert_eq!(m.state["doc"]["params"]["values"]["product"], "BROK");
    }

    /// Requests the report can't serve are refused with the reason and a status: unknown report or
    /// client (404), outside the caller's access (403), a parameter that doesn't fit (422).
    #[test]
    fn refuses_what_it_cant_make() {
        if dbconn::demo_path().is_none() { return }
        let app = banker();
        let (id, _) = own_client(&app);
        let q = |v: Json| make(&app, &ask(v), d("2026-11-10")).map(|_| ()).unwrap_err();
        let per = json!({"preset": "last_quarter"});
        assert_eq!(q(json!({"template": "nope", "client_id": id, "period": per})).status, 404);
        assert_eq!(q(json!({"client_id": "C9999", "period": per})).status, 404);
        let other = auth::options(&app.store()).unwrap()["clients"].as_array().unwrap().iter()
            .map(|c| c["id"].as_str().unwrap().to_string()).find(|c| !client_ok(app.clients.as_ref().unwrap(), c)).unwrap();
        let r = q(json!({"client_id": other, "period": per}));
        assert_eq!(r.status, 403, "{r:?}");
        assert_eq!(q(json!({"client_id": id, "period": per, "params": {"colour": "red"}})).status, 422);
        assert_eq!(q(json!({"client_id": id, "period": per, "params": {"client": "X"}})).status, 422);
        assert_eq!(q(json!({"client_id": id, "period": per, "params": {"product": ["NOPE"]}})).status, 422);
        assert_eq!(q(json!({"client_id": "", "period": per})).status, 422);
        // a parameter the API isn't let set, and a report kept off the API
        let app2 = banker_with(|st| { st.api.reports.clear(); });
        let r = make(&app2, &ask(json!({"client_id": id, "period": per, "params": {"product": ["BROK"]}})), d("2026-11-10")).map(|_| ()).unwrap_err();
        assert!(r.status == 422 && r.message.contains("can't be set over the API"), "{r:?}");
        let app3 = banker_with(|st| { st.api.reports.get_mut("uhnwi").unwrap().off = true; });
        assert_eq!(make(&app3, &ask(json!({"client_id": id, "period": per})), d("2026-11-10")).map(|_| ()).unwrap_err().status, 403);
        // nobody signed in
        let anon = app_with(sample::blank());
        assert_eq!(make(&anon, &ask(json!({"client_id": id, "period": per})), d("2026-11-10")).map(|_| ()).unwrap_err().status, 403);
        // unknown fields are a mistake, not ignored
        assert!(serde_json::from_value::<Request>(json!({"client_id": id, "period": per, "colour": 1})).is_err());
    }

    /// The pages are drawn without a browser by the app's own drawing code, and make a PDF: every
    /// shown page, with the client's name and the report's numbers in it.
    #[cfg(feature = "render")]
    #[test]
    fn draws_the_report_without_a_browser() {
        if dbconn::demo_path().is_none() { return }
        let app = banker();
        let (id, name) = own_client(&app);
        let m = make(&app, &ask(json!({"client_id": id, "period": {"start": "2026-07-01", "end": "2026-09-30"}})), d("2026-11-10")).unwrap();
        let ui = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui");
        let t = std::time::Instant::now();
        let out = draw(&ui, &m).unwrap();
        let first = t.elapsed();
        let shown = m.state["pages"].as_array().unwrap().iter().filter(|p| p["hidden"] != true).count();
        assert_eq!(out.pages, shown);
        assert!(out.pdf.starts_with(b"%PDF") && out.pdf.len() > 50_000, "{}", out.pdf.len());
        assert!(out.name.starts_with(&name), "{}", out.name);
        // the second report on this thread reuses the engine
        let t = std::time::Instant::now();
        let svgs = crate::render_js::pages(&ui, &m.state).unwrap();
        eprintln!("drawn {} pages: first {first:?} (engine started), again {:?}; {} layout issues", svgs.len(), t.elapsed(), out.issues);
        assert!(svgs.iter().all(|s| s.starts_with("<svg")));
        // the report's figures, written as Russian writes them (ICU's data in the engine)
        let grouped = |s: &str| s.as_bytes().windows(4).any(|w| w[0].is_ascii_digit() && w[1..3] == [0xC2, 0xA0] && w[3].is_ascii_digit());
        assert!(svgs.iter().any(|s| grouped(s)), "figures grouped by a no-break space");
        // for dev/shots/api-render.mjs: the same pages drawn in Chrome, compared
        let dump = std::env::temp_dir().join("quire_api_report");
        std::fs::write(dump.with_extension("pdf"), &out.pdf).unwrap();
        std::fs::write(dump.with_extension("svg"), svgs.join("\n")).unwrap();
        std::fs::write(dump.with_extension("json"), serde_json::to_string(&m.state).unwrap()).unwrap();
    }

    /// A system signs in with its API key and acts with its account's access: its own clients and
    /// reports only; Admin's defaults fill in what a request leaves out.
    #[test]
    fn api_accounts_act_with_their_own_access() {
        if dbconn::demo_path().is_none() { return }
        let mut key = String::new();
        let mut app = banker_with(|st| {
            let e = crate::api_accounts::ApiClientEdit { name: "CRM".into(), active: true, access: auth::Access { all: false, coverage: vec!["S0001".into()], ..Default::default() }, ..Default::default() };
            key = crate::api_accounts::save(st, e, "admin").unwrap().unwrap();
            st.api.reports.get_mut("uhnwi").unwrap().params.get_mut("product").unwrap().default = json!(["BROK", "BANK"]);
        });
        let (_, session) = crate::api_accounts::sign_in(&app.store(), &key).unwrap();
        app.end_session();
        app.start_session(session).unwrap();
        let (id, _) = own_client(&app);
        let per = json!({"preset": "last_quarter"});
        let m = make(&app, &ask(json!({"client_id": id, "period": per})), d("2026-11-10")).unwrap();
        assert_eq!(m.used["params"]["product"], json!(["BROK", "BANK"]), "Admin's default");
        let m = make(&app, &ask(json!({"client_id": id, "period": per, "params": {"product": "BANK"}})), d("2026-11-10")).unwrap();
        assert_eq!(m.used["params"]["product"], json!(["BANK"]), "the request's own");
        // another banker's client
        let other = auth::options(&app.store()).unwrap()["clients"].as_array().unwrap().iter()
            .map(|c| c["id"].as_str().unwrap().to_string()).find(|c| !client_ok(app.clients.as_ref().unwrap(), c)).unwrap();
        assert_eq!(make(&app, &ask(json!({"client_id": other, "period": per})), d("2026-11-10")).map(|_| ()).unwrap_err().status, 403);
        // only the reports Admin gave it
        let mut st = app.store();
        st.api.clients[0].reports = vec!["other".into()];
        auth::save(app.store_path.as_ref().unwrap(), &st).unwrap();
        assert_eq!(make(&app, &ask(json!({"client_id": id, "period": per})), d("2026-11-10")).map(|_| ()).unwrap_err().status, 403);
    }

    /// Admin → API: only an administrator manages it; a new account's key is shown once and is
    /// nowhere in the store or the activity log; a report's settings and the retention are checked.
    #[test]
    fn admin_manages_api_accounts() {
        let mut app = app_with(sample::blank());
        app.store_path = Some(store_with(|_| {}));
        let banker = auth::sign_in(&app.store(), "user", auth::DEMO_PASSWORD).unwrap();
        app.start_session(banker).unwrap();
        assert!(auth_cmd(&mut app, "api", &json!({})).is_err(), "not for a banker");
        app.end_session();
        app.start_session(auth::sign_in(&app.store(), "admin", auth::DEMO_PASSWORD).unwrap()).unwrap();
        let r = auth_cmd(&mut app, "api_save_client", &json!({"client": {"name": "Client portal", "active": true, "access": {"all": true}}})).unwrap();
        let key = r["key"].as_str().unwrap().to_string();
        let secret = key.rsplit('.').next().unwrap();
        assert_eq!(r["api"]["clients"][0]["id"], "client-portal");
        assert!(r["api"]["clients"][0].get("key_hash").is_none());
        let uhnwi = r["api"]["reports"].as_array().unwrap().iter().find(|x| x["id"] == "uhnwi").unwrap().clone();
        let product = uhnwi["params"].as_array().unwrap().iter().find(|p| p["name"] == "product").unwrap();
        assert_eq!(product["accept"], true);
        assert!(product["choices"].as_array().is_some_and(|c| c.iter().any(|x| x == "BROK")) || dbconn::demo_path().is_none());
        // a change keeps the key; a new key replaces it
        let r = auth_cmd(&mut app, "api_save_client", &json!({"client": {"was": "client-portal", "name": "Client portal", "active": true, "reports": ["uhnwi"]}})).unwrap();
        assert!(r["key"].is_null());
        let key2 = auth_cmd(&mut app, "api_new_key", &json!({"id": "client-portal"})).unwrap()["key"].as_str().unwrap().to_string();
        assert!(api_accounts::sign_in(&app.store(), &key).is_err() && api_accounts::sign_in(&app.store(), &key2).is_ok());
        // a report's settings
        let v = auth_cmd(&mut app, "api_save_report", &json!({"id": "uhnwi", "settings": {"params": {"product": {"accept": true, "default": "BROK"}}}})).unwrap();
        let product = v["reports"].as_array().unwrap().iter().find(|x| x["id"] == "uhnwi").unwrap()["params"].as_array().unwrap().iter().find(|p| p["name"] == "product").unwrap().clone();
        assert_eq!(product["api_default"], json!(["BROK"]));
        assert!(auth_cmd(&mut app, "api_save_report", &json!({"id": "uhnwi", "settings": {"params": {"nope": {"accept": true}}}})).is_err());
        assert!(auth_cmd(&mut app, "api_retention", &json!({"days": 0})).is_err());
        assert_eq!(auth_cmd(&mut app, "api_retention", &json!({"days": 30})).unwrap()["retention_days"], 30);
        // the secret is nowhere on disk
        let dir = app.audit_dir().unwrap();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            if e.path().is_file() {
                let t = std::fs::read_to_string(e.path()).unwrap_or_default();
                assert!(!t.contains(secret) && !t.contains(key2.rsplit('.').next().unwrap()), "a key in {}", e.path().display());
            }
        }
        auth_cmd(&mut app, "api_delete_client", &json!({"id": "client-portal"})).unwrap();
        assert!(api_accounts::sign_in(&app.store(), &key2).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
