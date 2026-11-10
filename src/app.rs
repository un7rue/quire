

use layout::{Op, Rect};
use model::{CatalogItem, Dataset, Doc, DocKind, Page, ParamDef, Params, Widget};
use python::PyEngine;
use query::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use crate::host::{Host, H};
use crate::{ai, ai_admin, ai_design, api_accounts, api_delivery, ai_report, ai_restyle, audit, auth, client_sample, data, dbconn, export, fonts, layout, looks, model, overview, period, proposal, proposals, python, query, sample, theme_studio, uhnwi_sample, uhnwi_theme, vm};

mod report_api;
#[cfg(feature = "render")]
pub(crate) use report_api::{ApiDesk, PeriodAsked, Refusal, Request};
#[cfg(feature = "render")]
pub(crate) use report_api::{draw, Drawn};

const UNDO_LIMIT: usize = 100;
/// Documents open at once (templates, reports and themes).
const MAX_OPEN: usize = 10;

/// An open document that isn't the one shown: kept with its file, changes and history.
struct Slot {
    id: u64,
    doc: Doc,
    path: Option<PathBuf>,
    dirty: bool,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    /// A theme studio's document (or the document's studio): their themes travel between them.
    link: Option<u64>,
}

struct App {
    doc: Option<Doc>,
    path: Option<PathBuf>,
    dirty: bool,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    engine: Engine,
    py: PyEngine,
    user: String,
    /// A one-off message for the UI (shown as a toast).
    notice: Option<String>,
    /// The other open documents, in the order they were opened.
    parked: Vec<Slot>,
    /// The shown document's id, and the document its theme studio belongs to (for a studio).
    active_id: u64,
    active_link: Option<u64>,
    next_id: u64,
    /// The studio's theme when it was last shown: only a changed theme travels to its document.
    studio_base: Option<Json>,
    /// This computer's settings: documents follow their formats unless they set their own.
    settings: Settings,
    /// Who is signed in (None before sign-in).
    session: Option<auth::Session>,
    /// The clients the signed-in person may report on, as (id, name); None = every client.
    clients: Option<Vec<(String, String)>>,
    /// Where accounts and the Data area's connections are kept (None: not kept, for tests).
    store_path: Option<PathBuf>,
    /// The client and period the Data area previews and runs SQL with while no
    /// report is open; they follow the last document opened.
    data_params: Params,
    /// A published report whose own data the Data area shows while no document is open.
    data_report: Option<String>,
    /// Work sessions: one per open document and client (see `App::work`).
    work: std::cell::RefCell<Works>,
}

/// The open work sessions, by open document; time counts for the one in front.
#[derive(Default)]
struct Works {
    open: std::collections::HashMap<u64, Work>,
}

struct Work {
    id: String,
    client: String,
    user: String,
    /// Seconds someone was working in it (the window in front, input in the last minutes).
    active_s: f64,
    /// Active seconds already written (a time-keeping line every five minutes).
    written_s: f64,
    /// Something happened in it besides opening (an edit, a save, an export, AI).
    busy: bool,
    /// When time was last counted: a ping never counts more than the time since.
    last: std::time::Instant,
    /// When it last saw an event or a ping (an admin session left for 30 minutes ends).
    seen: std::time::Instant,
}

/// The open-sessions key of the administrator's work (documents have their own ids).
const ADMIN_WORK: u64 = u64::MAX;

/// The core as the desktop app and the web server hold it: the app's state and its platform.
pub struct AppState {
    app: Mutex<App>,
    host: Box<dyn Host>,
}
type St<'a> = &'a AppState;

fn lock(s: &AppState) -> MutexGuard<'_, App> {
    s.app.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    /// A fresh app on `host`, signed out, with the settings and accounts kept in the host's folder.
    pub fn new(host: Box<dyn Host>, user: Option<String>) -> Self {
        let mut app = App {
            doc: None,
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            engine: Engine::default(),
            py: PyEngine::new(),
            user: user.unwrap_or_else(user_name),
            notice: None,
            parked: Vec::new(),
            active_id: 0,
            active_link: None,
            next_id: 0,
            studio_base: None,
            settings: Settings::default(),
            session: None,
            clients: None,
            store_path: None,
            data_params: Params::default(),
            data_report: None,
            work: Default::default(),
        };
        // the settings this computer saved, for every document opened from now on
        let s = read_settings(settings_path(&*host).as_deref());
        app.py.set_python(Some(s.python.clone()));
        app.settings = s;
        // accounts (and the Data area's connections) next to the settings
        app.store_path = host.data_dir().filter(|d| std::fs::create_dir_all(d).is_ok()).map(|d| d.join("users.json"));
        AppState { app: Mutex::new(app), host }
    }

    fn host(&self) -> H<'_> {
        &*self.host
    }

    /// Whether the document in front has unsaved changes.
    pub fn dirty(&self) -> bool {
        lock(self).dirty
    }

    /// Forgets unsaved changes (the person chose to close without saving).
    pub fn discard_changes(&self) {
        lock(self).dirty = false;
    }

    /// Ends every work session (the app is closing), with its time.
    pub fn end_work(&self, reason: &str) {
        lock(self).end_work_of(None, reason);
    }
}

/// A short random number (session ids).
fn rand_u32() -> u32 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.finish() as u32
}

/// A value in a change line: short, without JSON quotes.
fn short_val(v: &Json) -> String {
    let s = match v { Json::Null => "—".to_string(), Json::String(x) => x.clone(), x => x.to_string() };
    if s.chars().count() > 40 { format!("{}…", s.chars().take(40).collect::<String>()) } else { s }
}

/// Seconds as people read them: "1 h 05 min", "12 min", "40 s".
fn fmt_secs(s: f64) -> String {
    let s = s.round() as i64;
    if s >= 3600 { format!("{} h {:02} min", s / 3600, (s % 3600) / 60) } else if s >= 60 { format!("{} min", s / 60) } else { format!("{s} s") }
}

fn user_name() -> String {
    std::process::Command::new("id")
        .arg("-F")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "Me".into())
}

impl App {
    fn base_dir(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf))
    }

    /// Sets the shown document aside (it stays open).
    fn park(&mut self) {
        // leaving a theme studio (for any document): a changed theme goes to its document
        if let (Some(d), Some(l)) = (self.doc.as_ref().filter(|d| d.kind == DocKind::Theme), self.active_link) {
            if serde_json::to_value(&d.theme).ok() != self.studio_base {
                let (theme, fonts) = (d.theme.clone(), d.fonts.clone());
                if let Some(t) = self.parked.iter_mut().find(|x| x.id == l && x.doc.kind != DocKind::Theme) {
                    t.undo.push(t.doc.clone());
                    theme_studio::apply(&mut t.doc, &theme, &fonts);
                    looks::doc(&mut t.doc);
                    t.dirty = true;
                }
                self.studio_base = Some(serde_json::to_value(&theme).unwrap_or_default());
            }
        }
        if let Some(doc) = self.doc.take() {
            self.parked.push(Slot {
                id: self.active_id,
                doc,
                path: self.path.take(),
                dirty: self.dirty,
                undo: std::mem::take(&mut self.undo),
                redo: std::mem::take(&mut self.redo),
                link: self.active_link.take(),
            });
        }
    }

    /// Whether another document can open (all places taken by unsaved ones: no).
    fn room(&self) -> Result<(), String> {
        let open = self.parked.len() + usize::from(self.doc.is_some());
        if open >= MAX_OPEN && self.parked.iter().all(|s| s.dirty) && (self.doc.is_none() || self.dirty) {
            return Err(format!("{MAX_OPEN} documents are open with unsaved changes. Save or close one first."));
        }
        Ok(())
    }

    /// The open document saved at this file, if any.
    fn open_at(&self, p: &Path) -> Option<u64> {
        if self.path.as_deref() == Some(p) && self.doc.is_some() {
            return Some(self.active_id);
        }
        self.parked.iter().find(|s| s.path.as_deref() == Some(p)).map(|s| s.id)
    }

    /// Room for one more document: the oldest saved one closes when all places are taken.
    fn make_room(&mut self) -> Result<(), String> {
        while self.parked.len() + 1 > MAX_OPEN {
            let Some(i) = self.parked.iter().position(|s| !s.dirty) else {
                return Err(format!("{MAX_OPEN} documents are open with unsaved changes. Save or close one first."));
            };
            let gone = self.parked.remove(i).id;
            self.unlink(gone);
        }
        Ok(())
    }

    fn unlink(&mut self, id: u64) {
        for s in &mut self.parked {
            if s.link == Some(id) {
                s.link = None;
            }
        }
        if self.active_link == Some(id) {
            self.active_link = None;
        }
    }

    /// After the shown document changes: data, fonts, fields and table pieces follow it.
    fn refresh_doc(&mut self) {
        if let Some(d) = &self.doc {
            if d.kind != DocKind::Theme {
                self.data_params = d.params.clone();
            }
        }
        self.sync_library();
        self.engine.clear();
        self.py.clear_cache();
        self.register_fonts();
        self.refresh_fields();
        self.sync_chains();
    }

    /// Shows another open document. A theme studio and its document hand their theme
    /// over: to the document the studio's, to the studio the document's own edits.
    fn activate(&mut self, id: u64) -> Result<(), String> {
        if self.doc.is_some() && self.active_id == id {
            return Ok(());
        }
        if !self.parked.iter().any(|s| s.id == id) {
            return Err("That document is no longer open.".into());
        }
        self.park();
        let mut s = self.parked.remove(self.parked.iter().position(|s| s.id == id).ok_or("That document is no longer open.")?);
        // entering a studio: it takes its document's theme (edits made in the document's Theme tab)
        if s.doc.kind == DocKind::Theme {
            if let Some(d) = s.link.and_then(|l| self.parked.iter().find(|x| x.id == l)) {
                s.doc.theme = d.doc.theme.clone();
                s.doc.fonts = d.doc.fonts.clone();
            }
            self.studio_base = serde_json::to_value(&s.doc.theme).ok();
        }
        self.doc = Some(s.doc);
        self.path = s.path;
        self.dirty = s.dirty;
        self.undo = s.undo;
        self.redo = s.redo;
        self.active_id = s.id;
        self.active_link = s.link;
        self.refresh_doc();
        Ok(())
    }

    /// An open document's title (for the activity log).
    fn open_title(&self, id: u64) -> String {
        let title = |d: &Doc| if d.kind == DocKind::Theme { d.theme.name.clone() } else { d.meta.title.clone() };
        if self.active_id == id {
            self.doc.as_ref().map(title).unwrap_or_default()
        } else {
            self.parked.iter().find(|s| s.id == id).map(|s| title(&s.doc)).unwrap_or_default()
        }
    }

    /// Closes an open document (the shown one, or another); the last one opened shows next.
    fn close_doc(&mut self, id: u64) {
        if self.doc.is_some() && self.active_id == id {
            self.doc = None;
            self.path = None;
            self.dirty = false;
            self.undo.clear();
            self.redo.clear();
            self.active_link = None;
            self.unlink(id);
            if let Some(last) = self.parked.last().map(|s| s.id) {
                let _ = self.activate(last);
            }
        } else {
            self.parked.retain(|s| s.id != id);
            self.unlink(id);
        }
    }

    /// The open documents for the rail: the shown one and the others, in opening order.
    fn open_docs(&self) -> Json {
        let title = |d: &Doc, p: &Option<PathBuf>| -> String {
            if d.kind == DocKind::Theme {
                d.theme.name.clone()
            } else {
                p.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| d.meta.title.clone())
            }
        };
        let kind = |d: &Doc| match d.kind { DocKind::Theme => "theme", DocKind::Report => "report", _ => "template" };
        // who a report is for and its period, for the rail
        let about = |d: &Doc| if d.kind == DocKind::Theme { (String::new(), String::new()) } else { (Self::doc_client(d), period::label(&d.params)) };
        let mut all: Vec<Json> = self.parked.iter().map(|s| { let (c, p) = about(&s.doc); json!({"id": s.id, "kind": kind(&s.doc), "title": title(&s.doc, &s.path), "client": c, "period": p, "dirty": s.dirty, "active": false, "link": s.link}) }).collect();
        if let Some(d) = &self.doc {
            let (c, p) = about(d);
            all.push(json!({"id": self.active_id, "kind": kind(d), "title": title(d, &self.path), "client": c, "period": p, "dirty": self.dirty, "active": true, "link": self.active_link}));
        }
        all.sort_by_key(|x| x["id"].as_u64().unwrap_or(0));
        Json::Array(all)
    }

    fn load(&mut self, mut doc: Doc, path: Option<PathBuf>) {
        // the shown document stays open beside the new one
        self.park();
        let _ = self.make_room();
        self.next_id += 1;
        self.active_id = self.next_id;
        self.active_link = None;
        doc.normalize();
        doc.access_src = self.session.as_ref().map(|s| s.products()).unwrap_or_default();
        looks::doc(&mut doc);
        uhnwi_theme::fold_views(&mut doc);
        // a theme saved before colours could be named: repeats of its named colours follow them by name
        doc.theme.link_colours();
        // dividers between two halves go: the gap tells them apart (halves of 12 and 12)
        doc.remove_split_dividers();
        // charts of monthly values and their running total not set per series: on two axes
        doc.split_series_axes();
        // a proposal made when a report held only one takes an id; its views are the theme's
        proposal::migrate(&mut doc);
        proposal::fit_views(&mut doc);
        // a template takes the library's newer versions of its widget types (a report keeps what it was published with)
        if doc.kind == DocKind::Template {
            if let Some(dir) = widget_types_dir(self) {
                let ids: Vec<String> = doc.theme.widget_types.keys().cloned().collect();
                for id in ids {
                    if let Some(t) = read_widget_type(&dir, &id) {
                        if t.version > doc.theme.widget_types[&id].version { doc.theme.widget_types.insert(id, t); }
                    }
                }
            }
        }
        if doc.kind != DocKind::Theme {
            self.data_params = doc.params.clone();
        }
        self.doc = Some(doc);
        self.path = path;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.sync_library();
        self.engine.clear();
        self.py.clear_cache();
        self.register_fonts();
        self.refresh_fields();
        self.sync_chains();
    }

    /// Makes the document's embedded fonts available for measuring and export.
    fn register_fonts(&mut self) {
        use base64::Engine as _;
        let Some(doc) = &self.doc else { return };
        for f in &doc.fonts {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&f.data) {
                let _ = fonts::add(bytes);
            }
        }
    }

    /// Fills in dataset schemas that are missing (files saved without a
    /// preview, generated samples) so the inspector can offer fields.
    fn refresh_fields(&mut self) {
        self.refresh_fields_of(None);
    }

    /// Re-reads the schema of one dataset (`Some("")` = the newest) or of
    /// every dataset that has none yet.
    fn refresh_fields_of(&mut self, only: Option<&str>) {
        let base = self.base_dir();
        let App { doc, engine, py, .. } = self;
        let Some(doc) = doc.as_mut() else { return };
        let target = only.map(|id| if id.is_empty() { doc.datasets.last().map(|d| d.id.clone()).unwrap_or_default() } else { id.to_string() });
        for i in 0..doc.datasets.len() {
            let wanted = match &target {
                Some(t) => &doc.datasets[i].id == t,
                None => doc.datasets[i].fields.is_empty(),
            };
            if !wanted {
                continue;
            }
            let ds = doc.datasets[i].clone();
            if let Ok(t) = engine.table(doc, &ds, base.as_deref(), py) {
                doc.datasets[i].fields = t.fields();
            }
        }
    }

    fn results(&mut self) -> std::collections::HashMap<String, Json> {
        if let Some(d) = &self.doc {
            period::set_locale(&d.theme.locale);
            period::set_formats(&d.formats.over(&self.settings.formats, &d.theme.locale));
        }
        self.resolve_lookups();
        let base = self.base_dir();
        match &self.doc {
            Some(doc) => self.engine.results(doc, base.as_deref(), &mut self.py),
            None => Default::default(),
        }
    }

    /// Resolves every `{{data:…}}` placeholder the document uses.
    fn resolve_lookups(&mut self) {
        let base = self.base_dir();
        let App { doc, engine, py, .. } = self;
        let Some(doc) = doc.as_ref() else { return };
        let mut texts: Vec<&str> = vec![&doc.meta.title, &doc.meta.subtitle];
        // text variables may hold data placeholders too
        texts.extend(doc.variables.iter().filter(|v| v.kind != "sql" && v.kind != "data").map(|v| v.value.as_str()));
        for band in [&doc.page.header, &doc.page.footer] {
            for slot in [&band.left, &band.center, &band.right] {
                texts.push(&slot.text);
            }
        }
        for p in &doc.pages {
            texts.push(&p.title);
            texts.push(&p.description);
            texts.push(&p.show_if);
            for w in &p.widgets {
                texts.push(&w.title);
                texts.push(&w.subtitle);
                // every option text may carry placeholders (headings, notes, labels…)
                fn strings<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
                    match v {
                        serde_json::Value::String(s) if s.contains("{{") => out.push(s),
                        serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
                        serde_json::Value::Object(o) => o.values().for_each(|x| strings(x, out)),
                        _ => {}
                    }
                }
                strings(&w.opts, &mut texts);
                // column labels of tables may hold placeholders too
                if let Some(b) = &w.binding {
                    texts.extend(b.measures.iter().map(|m| m.label.as_str()).filter(|l| l.contains("{{")));
                }
            }
        }
        let mut map = std::collections::HashMap::new();
        for t in texts {
            for tok in vm::data_tokens(t) {
                if !map.contains_key(&tok) {
                    let v = engine.lookup(doc, &tok, base.as_deref(), py);
                    map.insert(tok, v);
                }
            }
            for q in vm::sql_tokens(t) {
                let key = format!("sql:{q}");
                if !map.contains_key(&key) {
                    let v = engine.sql_lookup(doc, &q, base.as_deref(), py);
                    map.insert(key, v);
                }
            }
        }
        // the designer's SQL and data variables, formatted as asked
        for v in doc.variables.iter().filter(|v| !v.name.trim().is_empty() && (v.kind == "sql" || v.kind == "data")) {
            // full precision, unless the variable leaves the value as it comes
            let raw = !v.format.is_empty();
            let raw = if v.kind == "sql" { engine.sql_lookup_as(doc, &query::bind_report(&v.value, doc), base.as_deref(), py, raw) } else { engine.lookup_as(doc, v.value.trim(), base.as_deref(), py, raw) };
            // and unformatted, for SQL (:name)
            let plain = if !v.format.is_empty() { raw.clone() } else if v.kind == "sql" { engine.sql_lookup_as(doc, &query::bind_report(&v.value, doc), base.as_deref(), py, true) } else { engine.lookup_as(doc, v.value.trim(), base.as_deref(), py, true) };
            map.insert(format!("varraw:{}", v.name.trim()), plain);
            map.insert(format!("var:{}", v.name.trim()), format_var(&raw, &v.format, doc));
        }
        vm::set_lookups(map);
    }

    /// Re-runs the fit engine on one page after an edit. `opts` overrides
    /// the document's arrange setting.
    fn arrange(&mut self, pi: usize, pinned: Option<&str>, opts: Option<layout::Options>, firm: bool) {
        let results = self.results();
        let Some(doc) = self.doc.as_mut() else { return };
        if pi >= doc.pages.len() {
            return;
        }
        let items = vm::items(doc, &doc.pages[pi], &results);
        let opts = opts.unwrap_or_else(|| doc.arrange.clone());
        let out = layout::arrange(items, vm::grid(doc), &opts, pinned, firm);
        for it in out {
            if let Some(w) = doc.pages[pi].widgets.iter_mut().find(|w| w.id == it.id) {
                (w.x, w.y, w.w, w.h) = (it.rect.x, it.rect.y, it.rect.w, it.rect.h);
            }
        }
    }

    fn resp(&mut self) -> Json {
        self.enforce_access();
        let results = self.results();
        let mut param_opts = {
            let base = self.base_dir();
            match &self.doc {
                Some(doc) if !doc.param_defs.is_empty() => self.engine.param_options(doc, base.as_deref(), &mut self.py),
                _ => json!({}),
            }
        };
        if let Some(doc) = self.doc.as_mut().filter(|d| d.kind == DocKind::Theme) {
            fit_studio(doc, &results);
        }
        let param_labels = {
            let base = self.base_dir();
            match &self.doc {
                Some(doc) if doc.param_defs.iter().any(|d| d.source.as_ref().is_some_and(|s| !s.label.is_empty())) => self.engine.param_labels(doc, base.as_deref(), &mut self.py),
                _ => json!({}),
            }
        };
        // the client choices are only the clients this person may see
        if let (Some(allowed), Some(doc), Some(o)) = (&self.clients, &self.doc, param_opts.as_object_mut()) {
            for d in doc.param_defs.iter().filter(|d| auth::is_client_param(d)) {
                if let Some(a) = o.get_mut(&d.name).and_then(|v| v.as_array_mut()) {
                    a.retain(|x| x.as_str().map(|v| client_ok(allowed, v)).unwrap_or(false));
                }
            }
        }
        let session = self.session_json();
        let Some(doc) = &self.doc else {
            return json!({ "doc": null, "user": self.user, "session": session, "python_off": python::check().is_err() });
        };
        period::set_locale(&doc.theme.locale);
        let p = &doc.params;
        let (pw, ph) = doc.page.dims();
        let file = self.path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string());
        let mut doc_json = serde_json::to_value(doc).unwrap_or(Json::Null);
        // font files travel separately (font_files); they'd bloat every update
        if let Some(fs) = doc_json.get_mut("fonts").and_then(|f| f.as_array_mut()) {
            for f in fs {
                if let Some(o) = f.as_object_mut() {
                    o.remove("data");
                }
            }
        }
        json!({
            "doc": doc_json,
            "fonts": vm::fonts_for(doc, None),
            "file": file,
            "path": self.path.as_ref().map(|p| p.display().to_string()),
            "dirty": self.dirty,
            "undo": !self.undo.is_empty(),
            "redo": !self.redo.is_empty(),
            "user": self.user,
            "session": session,
            "python_off": python::check().is_err(),
            "period": {
                "label": period::label(p),
                "compare": period::compare_label(p),
                "range": period::period(p),
                "compare_range": period::compare(p),
            },
            "page": {
                "w": pw, "h": ph,
                "content": doc.page.content(),
                "cell": doc.page.cell(),
                "cols": doc.page.cols, "rows": doc.page.rows, "gap": doc.page.gap,
                "margin": doc.page.margin,
            },
            "pages": vm::pages(doc, &results),
            "param_options": param_opts,
            "param_labels": param_labels,
            "formats": doc.formats.over(&self.settings.formats, &doc.theme.locale),
            "variables": variables_list(doc),
            "specs": vm::specs(),
            "header": vm::band(doc, &doc.page.header),
            "footer": vm::band(doc, &doc.page.footer),
            "notice": self.notice.take(),
            "open_docs": self.open_docs(),
            // a studio's document, and a document's open studio (for the switch buttons)
            "stash": self.active_link.and_then(|l| self.parked.iter().find(|s| s.id == l)).map(|s| s.doc.meta.title.clone()),
            "stash_kind": self.active_link.and_then(|l| self.parked.iter().find(|s| s.id == l)).map(|s| if s.doc.kind == DocKind::Report { "report" } else { "template" }),
            "studio_held": self.parked.iter().find(|s| s.doc.kind == DocKind::Theme && s.link == Some(self.active_id)).map(|s| s.doc.theme.name.clone()),
        })
    }

    /// After a designer adds or edits a dataset, moves the reporting period
    /// onto the data if the current period has none, so widgets aren't empty.
    fn snap_period(&mut self, id: &str) {
        let base = self.base_dir();
        let App { doc, engine, py, notice, .. } = self;
        let Some(doc) = doc.as_mut() else { return };
        if doc.kind != DocKind::Template {
            return;
        }
        let ds = if id.is_empty() { doc.datasets.last() } else { doc.dataset(id) };
        let Some(ds) = ds.cloned() else { return };
        let Some(df) = ds.date_field.clone().filter(|f| !f.is_empty()) else { return };
        let Ok(t) = engine.table(doc, &ds, base.as_deref(), py) else { return };
        let Some(c) = t.col(&df) else { return };
        let dates: Vec<chrono::NaiveDate> = t.rows.iter().filter_map(|r| r.get(c).and_then(|v| v.date())).collect();
        let Some(latest) = dates.iter().max().copied() else { return };
        let r = period::period(&doc.params);
        if dates.iter().any(|d| r.contains(*d)) {
            return;
        }
        doc.params.date = latest;
        if doc.params.grain == "custom" {
            doc.params.start = Some(period::add_months(latest, -3));
        }
        *notice = Some(format!(
            "Moved the reporting period to {}, the latest data in “{}”.",
            period::label(&doc.params),
            if ds.name.is_empty() { "the new source" } else { &ds.name }
        ));
    }

    /// Repeated pages: one copy of each repeated set (a page, or the pages of
    /// its group) per row of its data source. The designed pages show the
    /// first row; copies after them show the others. Copies are rebuilt from
    /// the data on every change, with stable ids (page id ~ copy number).
    fn sync_repeats(&mut self) {
        let has = self.doc.as_ref().is_some_and(|d| d.pages.iter().any(|p| !p.repeat.is_off() || !p.repeat_of.is_empty()));
        if !has {
            return;
        }
        // the data placeholders the data sources may use are current
        self.resolve_lookups();
        let base = self.base_dir();
        let App { doc, engine, py, .. } = self;
        let Some(doc) = doc.as_mut() else { return };
        // drop the copies, and the continuation pages of tables on them
        let copy_widgets: std::collections::HashSet<String> = doc.pages.iter().filter(|p| !p.repeat_of.is_empty()).flat_map(|p| p.widgets.iter().map(|w| w.id.clone())).collect();
        doc.pages.retain(|p| p.repeat_of.is_empty() && !(p.auto && p.widgets.iter().any(|w| vm::continues(w).is_some_and(|s| copy_widgets.contains(s)))));
        // the sets: pages that repeat, grouped by their group (a page without one is a set of its own)
        let mut sets: Vec<(String, model::Repeat, Vec<String>)> = Vec::new();
        for p in doc.pages.iter().filter(|p| !p.repeat.is_off() && !p.auto) {
            let key = if p.group.is_empty() { format!("#{}", p.id) } else { p.group.clone() };
            match sets.iter_mut().find(|(k, r, _)| *k == key && *r == p.repeat) {
                Some(set) => set.2.push(p.id.clone()),
                None => sets.push((key, p.repeat.clone(), vec![p.id.clone()])),
            }
        }
        for (_, rep, ids) in sets {
            let rows: Vec<std::collections::BTreeMap<String, String>> = match doc.dataset(&rep.dataset).cloned() {
                Some(ds) => engine.rows_text(doc, &ds, base.as_deref(), py).unwrap_or_default(),
                None => Vec::new(),
            };
            let rows: Vec<_> = rows
                .into_iter()
                .filter(|r| rep.field.is_empty() || rep.values.is_empty() || r.get(&rep.field).is_some_and(|v| rep.values.iter().any(|x| x == v)))
                .collect();
            let n = rows.len();
            // the designed pages show the first row
            for p in doc.pages.iter_mut().filter(|p| ids.contains(&p.id)) {
                p.row = rows.first().cloned().unwrap_or_default();
                p.copy = 0;
                p.copies = n;
            }
            if n < 2 {
                continue;
            }
            // copies go after the set (and the continuation pages of its tables)
            let set_widgets: std::collections::HashSet<String> = doc.pages.iter().filter(|p| ids.contains(&p.id)).flat_map(|p| p.widgets.iter().map(|w| w.id.clone())).collect();
            let mut at = doc
                .pages
                .iter()
                .rposition(|p| ids.contains(&p.id) || (p.auto && p.widgets.iter().any(|w| vm::continues(w).is_some_and(|s| set_widgets.contains(s)))))
                .unwrap_or(0);
            let sources: Vec<Page> = doc.pages.iter().filter(|p| ids.contains(&p.id)).cloned().collect();
            for (k, row) in rows.into_iter().enumerate().skip(1) {
                for src in &sources {
                    let mut c = src.clone();
                    c.id = format!("{}~{k}", src.id);
                    c.repeat = model::Repeat::default();
                    c.repeat_of = src.id.clone();
                    c.row = row.clone();
                    c.copy = k;
                    c.copies = n;
                    // each copy's pages link to each other, not to the other copies'
                    if !c.group.is_empty() {
                        c.group = format!("{}~{k}", src.group);
                    }
                    for w in &mut c.widgets {
                        w.id = format!("{}~{k}", w.id);
                        w.locked = true;
                        // a continuation inside the set continues this copy's table
                        if let Some(t) = vm::follows(w).filter(|t| set_widgets.contains(*t)).map(|t| format!("{t}~{k}")) {
                            w.opts["follows"] = json!(t);
                        }
                        if let Some(t) = vm::continues(w).filter(|t| set_widgets.contains(*t)).map(|t| format!("{t}~{k}")) {
                            w.opts["table"]["continues"] = json!(t);
                        }
                    }
                    at += 1;
                    doc.pages.insert(at, c);
                }
            }
        }
    }

    /// Keeps the continuation pieces of paginated tables in step with their
    /// data: adds pages when rows grow, removes them when rows shrink.
    fn sync_chains(&mut self) {
        self.sync_repeats();
        let has = self.doc.as_ref().is_some_and(|d| {
            d.pages.iter().flat_map(|p| &p.widgets).any(|w| vm::continues(w).is_some() || vm::paginated(&d.theme, w))
        });
        if !has {
            return;
        }
        let results = self.results();
        let Some(doc) = self.doc.as_mut() else { return };
        // linked widgets are worked out afresh: their mirrors go, the tables are their own height
        doc.grow.clear();
        doc.moved.clear();
        doc.keep.clear();
        for p in &mut doc.pages {
            p.widgets.retain(|w| w.opts.get("mirror").is_none());
        }
        let mut linked: Vec<(String, String, u32, Vec<Widget>)> = Vec::new();
        let theme = doc.theme.clone();
        let is_source = |w: &Widget| vm::paginated(&theme, w);
        let sources: Vec<(usize, Widget)> = doc
            .pages
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| p.widgets.iter().filter(|w| is_source(w)).map(move |w| (pi, w.clone())))
            .collect();
        let ids: std::collections::HashSet<String> = sources.iter().map(|(_, w)| w.id.clone()).collect();
        // pieces whose table is gone or no longer continues
        for p in &mut doc.pages {
            p.widgets.retain(|w| vm::continues(w).is_none_or(|s| ids.contains(s)));
        }
        for (_, src) in &sources {
            // where the source is now: earlier tables may have added pages before it
            let Some(spi) = doc.pages.iter().position(|p| p.widgets.iter().any(|w| w.id == src.id)) else { continue };
            let spi = &spi;
            // pieces mirror the source's data and look
            let mut cont_title = src.title.clone();
            if !cont_title.is_empty() {
                cont_title.push_str(continued_suffix(doc));
            }
            let rows = doc.page.rows;
            for p in &mut doc.pages {
                let auto = p.auto;
                for w in p.widgets.iter_mut().filter(|w| vm::continues(w) == Some(src.id.as_str())) {
                    // a piece on an automatic page starts each pass with the whole page again:
                    // the last one is trimmed to its rows below, and a trim must not outlive the rows it was for
                    if auto {
                        w.y = 0;
                        w.h = rows;
                    }
                    w.binding = src.binding.clone();
                    w.opts = continuation_opts(&src.opts, &src.id);
                    w.style = src.style.clone();
                    w.title = cont_title.clone();
                    w.subtitle = String::new();
                    w.locked = true;
                    w.x = src.x;
                    w.w = src.w;
                }
            }
            // widgets linked to it, under one of its pieces: while the table goes on past that piece,
            // the piece takes their rows and they go after the table's last rows
            let mut host: Option<(usize, Widget, Vec<Widget>)> = None;
            for (pi, p) in doc.pages.iter().enumerate() {
                for piece in p.widgets.iter().filter(|w| w.id == src.id || vm::continues(w) == Some(src.id.as_str())) {
                    let fs: Vec<Widget> = p.widgets.iter().filter(|w| vm::follows(w) == Some(src.id.as_str()) && w.y >= piece.y + piece.h).cloned().collect();
                    if !fs.is_empty() && host.is_none() {
                        host = Some((pi, piece.clone(), fs));
                    }
                }
            }
            if let Some((hpi, piece, followers)) = host {
                let data = results.get(&src.id);
                let n = data.and_then(|d| d.get("rows")).and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0) as u32;
                // the rows the pieces before it show
                let mut offset = 0u32;
                for (k, w) in std::iter::once(src).chain(doc.pages.iter().flat_map(|p| p.widgets.iter()).filter(|w| vm::continues(w) == Some(src.id.as_str()))).enumerate() {
                    if w.id == piece.id {
                        break;
                    }
                    let f = doc.page.frame(w.x, w.y, w.w, w.h);
                    offset += vm::widget_vm_chain(doc, w, f, data.cloned(), vm::Chain { offset, last: false, part: k as u32 }).table.map(|t| t.visible.max(1)).unwrap_or(1);
                }
                // the table ends on this piece at this height: everything fits with its totals,
                // or every data row does (only the totals would be left for the next page)
                let fits = |doc: &Doc, h: u32| {
                    let f = doc.page.frame(piece.x, piece.y, piece.w, h);
                    offset >= n
                        || vm::widget_vm_chain(doc, &piece, f, data.cloned(), vm::Chain { offset, last: true, part: 0 }).table.is_none_or(|t| offset + t.visible >= n)
                        || vm::widget_vm_chain(doc, &piece, f, data.cloned(), vm::Chain { offset, last: false, part: 0 }).table.is_none_or(|t| offset + t.visible >= n)
                };
                // every row fits, but the widgets linked to its end run past the page (the table was
                // made taller): its last row goes on to the next page, and they go after it there
                let overflow = followers.iter().any(|f| f.y + f.h > doc.page.rows);
                if overflow && fits(doc, piece.h) && n.saturating_sub(offset) >= 2 {
                    doc.keep.insert(piece.id.clone(), n - offset - 1);
                }
                if !fits(doc, piece.h) {
                    let ids: Vec<&str> = followers.iter().map(|f| f.id.as_str()).collect();
                    let bottom = followers.iter().map(|f| f.y + f.h).max().unwrap_or(piece.y + piece.h);
                    // never over another widget below it in its columns
                    let block = doc.pages[hpi].widgets.iter()
                        .filter(|o| o.id != piece.id && !ids.contains(&o.id.as_str()) && o.x < piece.x + piece.w && o.x + o.w > piece.x && o.y >= piece.y + piece.h)
                        .map(|o| o.y).min().unwrap_or(u32::MAX);
                    let limit = bottom.min(block).min(doc.page.rows);
                    // as much as it can take while some rows still go on: the linked widgets never end up alone
                    let mut g = limit.saturating_sub(piece.y + piece.h);
                    while g > 0 && fits(doc, piece.h + g) {
                        g -= 1;
                    }
                    doc.grow.insert(piece.id.clone(), g);
                    for f in &followers {
                        doc.moved.insert(f.id.clone());
                    }
                    linked.push((src.id.clone(), piece.id.clone(), piece.y + piece.h, followers));
                }
            }
            let mut extra = src.clone();
            extra.id = String::new();
            extra.y = 0;
            extra.h = rows;
            extra.title = cont_title.clone();
            extra.subtitle = String::new();
            extra.opts = continuation_opts(&src.opts, &src.id);
            let pieces: Vec<&Widget> = std::iter::once(src)
                .chain(doc.pages.iter().flat_map(|p| p.widgets.iter()).filter(|w| vm::continues(w) == Some(src.id.as_str())))
                .collect();
            let have = pieces.len();
            let need = vm::pieces_needed(doc, &pieces, &extra, results.get(&src.id));
            if need > have {
                let mut after = doc
                    .pages
                    .iter()
                    .rposition(|p| p.widgets.iter().any(|w| vm::continues(w) == Some(src.id.as_str())))
                    .unwrap_or(*spi);
                let title = format!("{}{}", strip_continued(&doc.pages[*spi].title), continued_suffix(doc));
                for _ in have..need {
                    let mut w = extra.clone();
                    w.id = doc.new_id("w");
                    w.locked = true;
                    let id = doc.new_id("p");
                    after += 1;
                    let section = doc.pages[*spi].section.clone();
                    let row = doc.pages[*spi].row.clone();
                    doc.pages.insert(after, Page { id, title: title.clone(), widgets: vec![w], auto: true, section, plain: false, row, ..Default::default() });
                }
            } else if need < have {
                // only automatic pages lose their pieces; a designed continuation page keeps its
                // piece and is left out of the report while that piece has no rows (see below)
                let designed = doc.pages.iter().filter(|p| !p.auto).flat_map(|p| &p.widgets).filter(|w| vm::continues(w) == Some(src.id.as_str())).count();
                let mut drop = (have - need).min(have.saturating_sub(1 + designed));
                for p in doc.pages.iter_mut().rev().filter(|p| p.auto) {
                    while drop > 0 {
                        match p.widgets.iter().rposition(|w| vm::continues(w) == Some(src.id.as_str())) {
                            Some(i) => {
                                p.widgets.remove(i);
                                drop -= 1;
                            }
                            None => break,
                        }
                    }
                }
            }
        }
        if !linked.is_empty() {
            place_linked(doc, &linked, &results);
        }
        if doc.pages.len() > 1 {
            doc.pages.retain(|p| !(p.auto && p.widgets.is_empty()));
        }
        // an automatic page belongs to the page its table starts on: its account (row), section and
        // title follow that page's — another client's accounts must never show on it
        for i in 0..doc.pages.len() {
            if !doc.pages[i].auto {
                continue;
            }
            let src = doc.pages[i].widgets.iter().find_map(|w| vm::continues(w).map(String::from));
            let Some(src) = src else { continue };
            let Some(spi) = doc.pages.iter().position(|p| p.widgets.iter().any(|w| w.id == src)) else { continue };
            let (row, section, title) = (doc.pages[spi].row.clone(), doc.pages[spi].section.clone(), format!("{}{}", strip_continued(&doc.pages[spi].title), continued_suffix(doc)));
            // (a proposal's actions going on: the page is that proposal's, out of the contents)
            let group = proposal::is_proposal(&doc.pages[spi]).then(|| doc.pages[spi].group.clone());
            let p = &mut doc.pages[i];
            p.row = row;
            p.section = section;
            p.title = title;
            if let Some(g) = group {
                p.group = g;
                p.toc_hidden = true;
            }
        }
        // a designed page that continues a table has nothing to show when the
        // table ended before it (a small account's holdings fit on one page): it is left out
        let ch = vm::chains(doc, &results);
        for p in doc.pages.iter_mut().filter(|p| !p.auto) {
            p.piece_empty = p.widgets.iter().any(|w| {
                vm::continues(w).is_some_and(|src| {
                    let rows = results.get(src).and_then(|r| r.get("rows")).and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0) as u32;
                    ch.get(&w.id).is_some_and(|c| c.offset >= rows)
                })
            });
        }
    }

    fn checkpoint(&mut self) {
        if let Some(d) = &self.doc {
            self.undo.push(d.clone());
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }
}

// ------------------------------------------------------------------ edits

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Edit {
    /// Several edits as one step (one undo).
    Batch { edits: Vec<Edit> },
    AddWidget { page: String, widget: Widget, at: Option<Rect> },
    /// A widget follows a widget type from now on (its own look moves into the type).
    AdoptWidgetType { id: String, wt: model::WidgetType },
    /// A widget leaves its type and keeps the look the type gave it.
    DetachWidgetType { id: String },
    /// A new widget made from a widget type.
    AddTypedWidget { page: String, wt: model::WidgetType, at: Option<Rect> },
    /// The document's copy of a widget type, replaced (its widgets follow).
    SetWidgetType { wt: model::WidgetType },
    AddFromCatalog { page: String, catalog: String, at: Option<Rect> },
    UpdateWidget { id: String, patch: Json },
    SetRect { id: String, rect: Rect },
    RemoveWidget { id: String },
    /// A part added to a composite widget (its place is found on the composite's grid).
    AddPart { id: String, part: Widget },
    ReplaceWidget { id: String, catalog: String },
    DuplicateWidget { id: String },
    MoveToPage { id: String, page: Option<String> },
    AddPage { after: Option<String> },
    RemovePage { page: String },
    RenamePage { page: String, title: String },
    SetPageSection { page: String, section: String },
    SetPagePlain { page: String, plain: bool },
    /// Page group, navigation label and frame.
    /// Repeats a page — and with `whole_group`, every page of its group — once per row of a data source (an empty dataset stops it).
    SetPageRepeat { page: String, repeat: model::Repeat, #[serde(default)] whole_group: bool },
    SetPageProps { page: String, group: Option<String>, label: Option<String>, frame: Option<String>, #[serde(default)] description: Option<String>, #[serde(default)] toc_hidden: Option<bool>, #[serde(default)] show_if: Option<String> },
    MovePage { page: String, to: usize },
    DuplicatePage { page: String },
    /// The design assistant's page: `page`'s widgets replaced by `widgets` (kept ones by id), or —
    /// without `page` — a new page after `after` holding them. One undo step.
    AiPage { page: Option<String>, after: Option<String>, title: String, widgets: Vec<Widget> },
    /// The banker's proposal page (see `proposal`): made or replaced from `spec`, before the legal text; `None` removes it.
    /// `id` names the proposal (a report may hold several): with `spec` it is made or replaced in
    /// place, without it removed; no `id` and no `spec` removes them all.
    SetProposal { #[serde(default)] id: Option<String>, spec: Option<Json>, #[serde(default)] notes: String, #[serde(default)] split: bool },
    SetMeta { patch: Json },
    /// Number, date and currency formats for this document.
    SetFormats { formats: model::Formats },
    /// The icons (logos) uploaded into this document.
    SetIcons { icons: Vec<model::IconFile> },
    /// The designer's own variables.
    SetVariables { variables: Vec<model::Variable> },
    SetPageSetup { patch: Json },
    SetTheme { patch: Json },
    SetPolicy { patch: Json },
    SetArrange { patch: Json },
    SetParams { params: Params },
    SetParamDefs { defs: Vec<ParamDef> },
    /// The document's database connections (all of them).
    SetConnections { connections: Vec<model::Connection> },
    UpsertDataset { dataset: Dataset },
    RemoveDataset { id: String },
    UpsertCatalog { item: CatalogItem },
    RemoveCatalog { id: String },
    SaveToCatalog { id: String, name: String, category: String, description: String },
    ApplyOps { page: String, ops: Vec<Op> },
    AutoArrange { page: String },
    /// Embeds a font file in the document.
    AddFont { path: String },
    RemoveFont { family: String },
    /// A new design view: a copy of `from`, or the look of `widget` (which then uses it).
    AddVariant { kind: String, name: String, from: Option<String>, widget: Option<String> },
    UpdateVariant { kind: String, id: String, name: Option<String>, description: Option<String>, sample: Option<String> },
    RemoveVariant { kind: String, id: String },
    MoveVariant { kind: String, id: String, to: usize },
}

use model::merge_json as merge;

fn patched<T: Serialize + for<'de> Deserialize<'de>>(v: &T, patch: &Json) -> Result<T, String> {
    let mut j = serde_json::to_value(v).map_err(|e| e.to_string())?;
    merge(&mut j, patch);
    serde_json::from_value(j).map_err(|e| format!("Invalid change: {e}"))
}

/// A composite widget type's parts reach every widget of the type (as its settings do).
fn give_parts(doc: &mut Doc, wt: &model::WidgetType) {
    if wt.parts.is_empty() {
        return;
    }
    for w in doc.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()).filter(|w| w.wtype == wt.id) {
        w.parts = wt.parts.clone();
    }
}

fn widget<'a>(doc: &'a Doc, id: &str) -> Result<&'a Widget, String> {
    // a composite's part answers to its composite's rules (locked with it, changed as it may be)
    let id = id.split_once("::").map(|(p, _)| p).unwrap_or(id);
    doc.find_widget(id).map(|(p, w)| &doc.pages[p].widgets[w]).ok_or_else(|| "That widget no longer exists.".into())
}

/// What end users may change in a report. Designers can do anything.
fn check_policy(doc: &Doc, e: &Edit) -> Result<(), String> {
    // end-user rules apply to reports; designers edit templates and themes freely
    if doc.kind != DocKind::Report {
        return Ok(());
    }
    let pol = &doc.policy;
    let deny = |msg: &str| Err(msg.to_string());
    let unlocked = |id: &str| -> Result<&Widget, String> {
        let w = widget(doc, id)?;
        if w.locked {
            Err("This part of the template is locked.".into())
        } else {
            Ok(w)
        }
    };
    // filters the template shows read-only (or hides) can't be changed in a report
    if let Edit::SetParams { params } = e {
        let p = &doc.params;
        if !model::can_change(&pol.period) && (params.grain != p.grain || params.date != p.date || params.start != p.start) {
            return deny("The template fixes the reporting period.");
        }
        if !model::can_change(&pol.compare) && params.compare != p.compare {
            return deny("The template fixes the comparison.");
        }
        for d in doc.param_defs.iter().filter(|d| !model::can_change(&d.user)) {
            if params.values.get(&d.name) != p.values.get(&d.name) {
                return deny(&format!("The template fixes “{}”.", if d.label.is_empty() { &d.name } else { &d.label }));
            }
        }
    }
    match e {
        Edit::AiPage { .. } => deny("Pages are designed in the template."),
        // the banker's own page: always theirs to add, change or remove
        Edit::SetProposal { .. } => Ok(()),
        Edit::AddWidget { widget, .. } => match widget.kind.as_str() {
            "comment" if pol.comment => Ok(()),
            // words, headings, lines and pictures need no data
            "text" | "heading" | "divider" | "image" if pol.add && widget.binding.is_none() => Ok(()),
            _ => deny("Add figures from this report's list."),
        },
        Edit::AddFromCatalog { .. } if !pol.add => deny("This template doesn't allow adding widgets."),
        // a figure that follows a repeated page's row belongs on that page
        Edit::AddFromCatalog { page, catalog, .. } => {
            let item = doc.catalog.iter().find(|c| &c.id == catalog).ok_or("That gallery item no longer exists.")?;
            let rowed = !item.page.is_empty() && serde_json::to_string(&item.widget).is_ok_and(|t| t.contains("{{row."));
            if rowed && designed_page(doc, page) != item.page {
                deny("This figure belongs to its account's page.")
            } else {
                Ok(())
            }
        }
        Edit::ReplaceWidget { id, .. } => {
            unlocked(id)?;
            if pol.add && pol.remove {
                Ok(())
            } else {
                deny("This template doesn't allow replacing widgets.")
            }
        }
        Edit::RemoveWidget { id } => {
            let w = unlocked(id)?;
            if pol.remove || (w.kind == "comment" && pol.comment) {
                Ok(())
            } else {
                deny("This template doesn't allow removing widgets.")
            }
        }
        Edit::SetRect { id, .. } | Edit::MoveToPage { id, .. } | Edit::DuplicateWidget { id } => {
            unlocked(id)?;
            if pol.arrange {
                Ok(())
            } else {
                deny("This template doesn't allow rearranging.")
            }
        }
        Edit::UpdateWidget { id, patch } => {
            let w = unlocked(id)?;
            let obj = patch.as_object().ok_or("Invalid change.")?;
            for (k, v) in obj {
                match k.as_str() {
                    "title" | "subtitle" if pol.restyle || pol.edit_text => {}
                    "style" if pol.restyle => {}
                    // a note's author picks its background
                    "style" if w.kind == "comment" && pol.comment => {}
                    // another prepared look, or the same data as another chart
                    "variant" if pol.restyle => {}
                    "kind" if pol.restyle && CHART_KINDS.contains(&w.kind.as_str()) && v.as_str().is_some_and(|k| CHART_KINDS.contains(&k)) => {}
                    "opts" => {
                        for ok in v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
                            let text_key = matches!(ok.as_str(), "text" | "author" | "date" | "subtitle" | "eyebrow" | "signed" | "arrows" | "mark");
                            let allowed = if text_key {
                                (w.kind == "comment" && pol.comment) || pol.edit_text
                            } else {
                                pol.restyle
                            };
                            if !allowed {
                                return deny("This template doesn't allow that change.");
                            }
                        }
                    }
                    "binding" => {
                        // users may change the time window and how many rows show, not the data
                        let keys: Vec<String> = v.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                        if !keys.iter().all(|k| matches!(k.as_str(), "window" | "limit" | "sort" | "others")) {
                            return deny("The data behind a widget is set by the template.");
                        }
                    }
                    _ => return deny("This template doesn't allow that change."),
                }
            }
            Ok(())
        }
        Edit::AddPage { .. } | Edit::RemovePage { .. } | Edit::MovePage { .. } | Edit::DuplicatePage { .. } | Edit::SetPageSection { .. } | Edit::SetPagePlain { .. } | Edit::SetPageProps { .. }
            if !pol.pages =>
        {
            deny("This template doesn't allow changing pages.")
        }
        Edit::RemovePage { page } => {
            let p = doc.pages.iter().find(|p| &p.id == page).ok_or("No such page.")?;
            if p.widgets.iter().any(|w| w.locked) {
                deny("This page is part of the template and can't be removed.")
            } else {
                Ok(())
            }
        }
        Edit::ApplyOps { ops, .. } => {
            for op in ops {
                match op {
                    Op::SetRect { id, .. } | Op::MoveToPage { id, .. } | Op::Remove { id } | Op::SetLimit { id, .. } | Op::Paginate { id } => {
                        unlocked(id)?;
                    }
                    Op::RemovePage { .. } if !pol.pages => return deny("This template doesn't allow changing pages."),
                    _ => {}
                }
            }
            Ok(())
        }
        Edit::SetMeta { patch } => {
            let ok = patch.as_object().is_some_and(|o| o.keys().all(|k| matches!(k.as_str(), "title" | "subtitle" | "author")));
            if ok {
                Ok(())
            } else {
                deny("Only the title, subtitle and author can change.")
            }
        }
        Edit::AdoptWidgetType { .. } | Edit::DetachWidgetType { .. } | Edit::AddTypedWidget { .. } | Edit::SetWidgetType { .. } => deny("Widget types are changed in the template."),
        Edit::SetPageSetup { .. }
        | Edit::SetTheme { .. }
        | Edit::SetFormats { .. }
        | Edit::SetIcons { .. }
        | Edit::SetVariables { .. }
        | Edit::SetPolicy { .. }
        | Edit::SetParamDefs { .. }
        | Edit::SetConnections { .. }
        | Edit::UpsertDataset { .. }
        | Edit::RemoveDataset { .. }
        | Edit::UpsertCatalog { .. }
        | Edit::RemoveCatalog { .. }
        | Edit::SaveToCatalog { .. }
        | Edit::AddFont { .. }
        | Edit::RemoveFont { .. }
        | Edit::AddVariant { .. }
        | Edit::UpdateVariant { .. }
        | Edit::RemoveVariant { .. }
        | Edit::MoveVariant { .. } => deny("Only the template designer can change this."),
        // a composite's parts are the template's design
        Edit::AddPart { .. } => deny("Only the template designer can change this."),
        _ => Ok(()),
    }
}

fn rect_items(doc: &Doc, page: &Page) -> Vec<layout::Item> {
    page.widgets
        .iter()
        // a divider drawn in a gap takes no room
        .filter(|w| !model::in_gap(&doc.theme, w))
        .map(|w| layout::Item {
            id: w.id.clone(),
            kind: w.kind.clone(),
            rect: Rect { x: w.x, y: w.y, w: w.w, h: w.h },
            locked: false,
            need_h: None,
            table_rows: None,
        })
        .collect()
}

/// Puts a widget on a page: at `at` if given, else the first free slot,
/// else below everything (the fit engine then makes room).
fn place(doc: &Doc, pi: usize, w: &mut Widget, at: Option<Rect>) {
    let grid = vm::grid(doc);
    w.w = w.w.clamp(1, grid.cols);
    if let Some(r) = at {
        (w.x, w.y, w.w, w.h) = (r.x, r.y, r.w.clamp(1, grid.cols), r.h.max(1));
        w.x = w.x.min(grid.cols - w.w);
        return;
    }
    let items = rect_items(doc, &doc.pages[pi]);
    match layout::find_slot(&items, grid, w.w, w.h) {
        Some(r) => (w.x, w.y) = (r.x, r.y),
        None => (w.x, w.y) = (0, layout::bottom(&items)),
    }
}

/// " (continued)", in the report's language.
fn continued_suffix(doc: &Doc) -> &'static str {
    if doc.theme.locale.starts_with("ru") { " (продолжение)" } else { " (continued)" }
}

/// A page title without its "continued" ending, in either language.
fn strip_continued(t: &str) -> &str {
    t.trim_end_matches(" (continued)").trim_end_matches(" (продолжение)")
}

fn new_page_after(doc: &mut Doc, pi: usize, title: String) -> usize {
    let id = doc.new_id("p");
    let section = doc.pages.get(pi).map(|p| p.section.clone()).unwrap_or_default();
    doc.pages.insert(pi + 1, Page { id, title, widgets: Vec::new(), auto: false, section, plain: false, ..Default::default() });
    pi + 1
}

/// Returns (from page, to page, whether the target page is new).
fn move_to_page(doc: &mut Doc, id: &str, target: Option<&str>) -> Result<(usize, usize, bool), String> {
    let (pi, wi) = doc.find_widget(id).ok_or("That widget no longer exists.")?;
    let ti = match target {
        Some(t) => doc.page_index(t).ok_or("No such page.")?,
        None => {
            let title = format!("{}{}", strip_continued(&doc.pages[pi].title), continued_suffix(doc));
            new_page_after(doc, pi, title)
        }
    };
    let mut w = doc.pages[pi].widgets.remove(wi);
    if ti == pi {
        doc.pages[pi].widgets.insert(wi, w);
        return Ok((pi, ti, false));
    }
    let fresh = target.is_none();
    if fresh {
        // a widget squeezed to fit its old page gets its natural size back
        let s = layout::spec(&w.kind);
        w.h = w.h.max(s.def_h);
    }
    place(doc, ti, &mut w, None);
    doc.pages[ti].widgets.push(w);
    Ok((pi, ti, fresh))
}

/// A page an edit changed, to be re-arranged afterwards.
struct Touch {
    page: usize,
    /// The widget the user just placed; it keeps its spot and size.
    pinned: Option<String>,
    /// Arrange with these options instead of the document's.
    opts: Option<layout::Options>,
    /// The user sized the pinned widget by hand: the engine never shrinks it back.
    firm: bool,
}

fn touch(page: usize, pinned: Option<String>) -> Touch {
    Touch { page, pinned, opts: None, firm: false }
}

/// Applies an edit; returns the pages to re-arrange.
fn apply(app: &mut App, e: Edit) -> Result<Vec<Touch>, String> {
    let user = app.user.clone();
    let doc = app.doc.as_mut().ok_or("No document is open.")?;
    if let Edit::Batch { edits } = e {
        let mut all = Vec::new();
        for e in edits {
            all.extend(apply(app, e)?);
        }
        return Ok(all);
    }
    check_policy(doc, &e)?;
    let mut touched: Vec<Touch> = Vec::new();
    match e {
        Edit::Batch { .. } => unreachable!("handled above"),
        Edit::AddWidget { page, mut widget, at } => {
            let pi = doc.page_index(&designed_page(doc, &page)).ok_or("No such page.")?;
            widget = doc.instantiate(widget);
            if widget.kind == "comment" {
                stamp_comment(&mut widget, &user);
            }
            if doc.kind == DocKind::Report {
                widget.locked = false;
            }
            let pin = at.is_some();
            place(doc, pi, &mut widget, at);
            let id = widget.id.clone();
            doc.pages[pi].widgets.push(widget);
            // a dropped widget keeps the size the user chose; an auto-placed one may grow
            touched.push(touch(pi, pin.then_some(id)));
        }
        Edit::AdoptWidgetType { id, wt } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("No such widget.")?;
            let w = &mut doc.pages[pi].widgets[wi];
            if w.kind != wt.kind {
                return Err("A widget type keeps its kind.".into());
            }
            // the type holds the look now; the widget keeps its data, words and place
            w.wtype = wt.id.clone();
            w.opts = json!({});
            w.style = json!({});
            w.variant = String::new();
            give_parts(doc, &wt);
            doc.theme.widget_types.insert(wt.id.clone(), wt);
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::DetachWidgetType { id } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("No such widget.")?;
            let w = doc.pages[pi].widgets[wi].clone();
            let Some(t) = doc.theme.widget_types.get(&w.wtype).cloned() else { return Err("This widget has no type.".into()) };
            let mut opts = if t.opts.is_object() { t.opts.clone() } else { json!({}) };
            model::merge_json(&mut opts, &w.opts);
            let mut style = if t.style.is_object() { t.style.clone() } else { json!({}) };
            model::merge_json(&mut style, &w.style);
            let x = &mut doc.pages[pi].widgets[wi];
            x.opts = opts;
            x.style = style;
            if x.variant.is_empty() { x.variant = t.variant.clone(); }
            x.wtype = String::new();
            touched.push(touch(pi, None));
        }
        Edit::AddTypedWidget { page, wt, at } => {
            let pi = doc.page_index(&designed_page(doc, &page)).ok_or("No such page.")?;
            let mut w = Widget { kind: wt.kind.clone(), title: wt.title.clone(), subtitle: wt.subtitle.clone(), binding: wt.binding.clone(),
                w: wt.w.max(1), h: wt.h.max(1), wtype: wt.id.clone(), parts: wt.parts.clone(), ..Default::default() };
            doc.theme.widget_types.insert(wt.id.clone(), wt);
            w = doc.instantiate(w);
            let pin = at.is_some();
            place(doc, pi, &mut w, at);
            let id = w.id.clone();
            doc.pages[pi].widgets.push(w);
            touched.push(touch(pi, pin.then_some(id)));
        }
        Edit::SetWidgetType { wt } => {
            give_parts(doc, &wt);
            doc.theme.widget_types.insert(wt.id.clone(), wt);
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::AddFromCatalog { page, catalog, at } => {
            let pi = doc.page_index(&designed_page(doc, &page)).ok_or("No such page.")?;
            let item = doc.catalog.iter().find(|c| c.id == catalog).ok_or("That gallery item no longer exists.")?;
            let mut w = item.widget.clone();
            w.catalog = Some(item.id.clone());
            w.locked = false;
            let mut w = doc.instantiate(w);
            if w.kind == "comment" {
                stamp_comment(&mut w, &user);
            }
            let pin = at.is_some();
            place(doc, pi, &mut w, at);
            let id = w.id.clone();
            doc.pages[pi].widgets.push(w);
            touched.push(touch(pi, pin.then_some(id)));
        }
        Edit::UpdateWidget { id, patch } if doc.kind == DocKind::Theme => {
            theme_studio::route_update(doc, &id, &patch)?;
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::AddVariant { kind, name, from, widget } => {
            let look = widget.as_ref().and_then(|wid| doc.find_widget(wid)).map(|(pi, wi)| {
                let e = model::effective(&doc.theme, &doc.pages[pi].widgets[wi]).into_owned();
                (e.opts, e.style)
            });
            let vid = theme_studio::add_variant(&mut doc.theme, &kind, &name, from.as_deref(), look.as_ref().map(|l| l.0.clone()), look.map(|l| l.1))?;
            if let Some((pi, wi)) = widget.as_ref().and_then(|wid| doc.find_widget(wid)) {
                // the widget now uses the view and keeps only its content
                let w = &mut doc.pages[pi].widgets[wi];
                let design = theme_studio::design_part(&w.opts);
                if let (Some(o), Some(d)) = (w.opts.as_object_mut(), design.as_object()) {
                    for k in d.keys() {
                        if k != "table" {
                            o.remove(k);
                        }
                    }
                    if let Some(t) = o.get_mut("table").and_then(|t| t.as_object_mut()) {
                        t.retain(|k, _| matches!(k.as_str(), "cols" | "order" | "continues" | "totalLabel"));
                    }
                }
                w.style = json!({});
                w.variant = vid;
                touched.push(touch(pi, None));
            }
            if doc.kind == DocKind::Theme {
                theme_studio::rebuild(doc);
                touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
            }
        }
        Edit::UpdateVariant { kind, id, name, description, sample } => {
            let sv = doc.theme.styles.get_mut(&kind).and_then(|l| l.iter_mut().find(|s| s.id == id)).ok_or("That design view no longer exists.")?;
            if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
                sv.name = n.trim().to_string();
            }
            if let Some(d) = description {
                sv.description = d.trim().to_string();
            }
            if let Some(x) = sample {
                sv.sample = x;
            }
            if doc.kind == DocKind::Theme {
                theme_studio::rebuild(doc);
            }
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::RemoveVariant { kind, id } => {
            theme_studio::remove_variant(&mut doc.theme, &kind, &id);
            for p in &mut doc.pages {
                for w in &mut p.widgets {
                    if w.kind == kind && w.variant == id && doc.kind != DocKind::Theme {
                        w.variant.clear();
                    }
                }
            }
            if doc.kind == DocKind::Theme {
                theme_studio::rebuild(doc);
            }
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::MoveVariant { kind, id, to } => {
            theme_studio::move_variant(&mut doc.theme, &kind, &id, to);
            if doc.kind == DocKind::Theme {
                theme_studio::rebuild(doc);
            }
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        // a composite's part ("parent::part"): the change is to the part, kept in the composite
        Edit::UpdateWidget { id, patch } if id.contains("::") => {
            let (pi, wi, k) = doc.find_part(&id).ok_or("That part no longer exists.")?;
            let part_id = doc.pages[pi].widgets[wi].parts[k].id.clone();
            let mut p = patched(&doc.pages[pi].widgets[wi].parts[k], &patch)?;
            p.id = part_id;
            p.parts = Vec::new();
            // the composite's data source, written back as it is shown, stays the composite's (it follows it)
            let shared = doc.pages[pi].widgets[wi].binding.as_ref().map(|b| b.dataset.clone()).unwrap_or_default();
            if let Some(b) = p.binding.as_mut().filter(|b| !shared.is_empty() && b.dataset == shared) {
                b.dataset.clear();
            }
            doc.pages[pi].widgets[wi].parts[k] = p;
            let parent = doc.pages[pi].widgets[wi].id.clone();
            touched.push(touch(pi, Some(parent)));
        }
        Edit::UpdateWidget { id, patch } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("That widget no longer exists.")?;
            let mut w = patched(&doc.pages[pi].widgets[wi], &patch)?;
            w.id = id.clone();
            doc.pages[pi].widgets[wi] = w;
            touched.push(touch(pi, Some(id)));
        }
        // a part moves and resizes on its composite's grid
        Edit::SetRect { id, rect } if id.contains("::") => {
            let (pi, wi, k) = doc.find_part(&id).ok_or("That part no longer exists.")?;
            let (cols, rows) = model::part_grid(&doc.pages[pi].widgets[wi]);
            let (w, h) = (rect.w.clamp(1, cols), rect.h.clamp(1, rows));
            let (x, y) = (rect.x.min(cols - w), rect.y.min(rows - h));
            // parts don't cover each other
            let parts = &doc.pages[pi].widgets[wi].parts;
            if parts.iter().enumerate().any(|(j, q)| j != k && x < q.x + q.w && q.x < x + w && y < q.y + q.h && q.y < y + h) {
                return Err("That place is taken by another part: move or shrink it first.".into());
            }
            let p = &mut doc.pages[pi].widgets[wi].parts[k];
            (p.x, p.y, p.w, p.h) = (x, y, w, h);
            let parent = doc.pages[pi].widgets[wi].id.clone();
            touched.push(touch(pi, Some(parent)));
        }
        Edit::SetRect { id, rect } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("That widget no longer exists.")?;
            let cols = doc.page.cols.max(1);
            let w = &mut doc.pages[pi].widgets[wi];
            let old_bottom = w.y + w.h;
            // placed by hand: later tidying leaves it where it is
            if doc.arrange.auto {
                if !w.opts.is_object() { w.opts = json!({}); }
                w.opts["placed"] = json!(true);
            }
            w.w = rect.w.clamp(1, cols);
            w.h = rect.h.max(1);
            w.x = rect.x.min(cols - w.w);
            w.y = rect.y;
            let new_bottom = w.y + w.h;
            // widgets kept with a table's end move with it, up or down, at the same distance
            if new_bottom != old_bottom {
                for f in doc.pages[pi].widgets.iter_mut().filter(|f| vm::follows(f) == Some(id.as_str()) && f.y >= old_bottom) {
                    f.y = (f.y as i64 + new_bottom as i64 - old_bottom as i64).max(new_bottom as i64) as u32;
                }
            }
            touched.push(Touch { firm: true, ..touch(pi, Some(id)) });
        }
        Edit::RemoveWidget { id } if id.contains("::") => {
            let (pi, wi, k) = doc.find_part(&id).ok_or("That part no longer exists.")?;
            doc.pages[pi].widgets[wi].parts.remove(k);
            let parent = doc.pages[pi].widgets[wi].id.clone();
            touched.push(touch(pi, Some(parent)));
        }
        // a new part in a composite: at the first free cells of its grid, or where it is put
        Edit::AddPart { id, mut part } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("That widget no longer exists.")?;
            let parent = &mut doc.pages[pi].widgets[wi];
            let (cols, rows) = model::part_grid(parent);
            let mut n = parent.parts.len() + 1;
            while parent.parts.iter().any(|p| p.id == format!("p{n}")) { n += 1; }
            part.id = format!("p{n}");
            part.parts = Vec::new();
            part.w = part.w.clamp(1, cols);
            part.h = part.h.clamp(1, rows);
            // the first place it fits without covering another part, reading across then down
            let taken = |x: u32, y: u32, w: u32, h: u32| parent.parts.iter().any(|q| x < q.x + q.w && q.x < x + w && y < q.y + q.h && q.y < y + h);
            let spot = (0..=rows - part.h).flat_map(|y| (0..=cols - part.w).map(move |x| (x, y))).find(|&(x, y)| !taken(x, y, part.w, part.h));
            let (x, y) = match spot {
                Some(s) => s,
                // no room: the grid grows by the part's rows, and it goes under the others
                None => {
                    if !parent.opts.is_object() { parent.opts = json!({}); }
                    parent.opts["grid"] = json!({"cols": cols, "rows": (rows + part.h).min(48)});
                    (0, rows.min(48 - part.h))
                }
            };
            part.x = x;
            part.y = y;
            parent.parts.push(part);
            let pid = parent.id.clone();
            touched.push(touch(pi, Some(pid)));
        }
        Edit::RemoveWidget { id } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("That widget no longer exists.")?;
            doc.pages[pi].widgets.remove(wi);
            touched.push(touch(pi, None));
        }
        Edit::ReplaceWidget { id, catalog } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("That widget no longer exists.")?;
            let item = doc.catalog.iter().find(|c| c.id == catalog).ok_or("That gallery item no longer exists.")?;
            let old = &doc.pages[pi].widgets[wi];
            let (x, y, w, h) = (old.x, old.y, old.w, old.h);
            let mut nw = item.widget.clone();
            nw.catalog = Some(item.id.clone());
            nw.locked = false;
            let mut nw = doc.instantiate(nw);
            if nw.kind == "comment" {
                stamp_comment(&mut nw, &user);
            }
            let s = layout::spec(&nw.kind);
            (nw.x, nw.y, nw.w, nw.h) = (x, y, w.max(s.min_w), h.clamp(s.min_h, s.max_h.max(s.min_h)));
            let nid = nw.id.clone();
            doc.pages[pi].widgets[wi] = nw;
            touched.push(touch(pi, Some(nid)));
        }
        Edit::DuplicateWidget { id } => {
            let (pi, wi) = doc.find_widget(&id).ok_or("That widget no longer exists.")?;
            let mut w = doc.pages[pi].widgets[wi].clone();
            w.locked = false;
            let mut w = doc.instantiate(w);
            place(doc, pi, &mut w, None);
            let nid = w.id.clone();
            doc.pages[pi].widgets.push(w);
            touched.push(touch(pi, Some(nid)));
        }
        Edit::MoveToPage { id, page } => {
            let (from, to, fresh) = move_to_page(doc, &id, page.as_deref())?;
            touched.push(touch(from, None));
            touched.push(touch(to, if fresh { None } else { Some(id) }));
        }
        Edit::AiPage { page, after, title, widgets } => {
            let pi = match page {
                Some(id) => {
                    let pi = doc.page_index(&id).ok_or("No such page.")?;
                    doc.pages[pi].widgets.clear();
                    if !title.is_empty() {
                        doc.pages[pi].title = title;
                    }
                    pi
                }
                None => {
                    let at = match after {
                        Some(a) => doc.page_index(&a).ok_or("No such page.")?,
                        None => doc.pages.len() - 1,
                    };
                    let n = doc.pages.len() + 1;
                    new_page_after(doc, at, if title.is_empty() { format!("Page {n}") } else { title })
                }
            };
            for w in widgets {
                let w = if w.id.is_empty() { doc.instantiate(w) } else { w };
                doc.pages[pi].widgets.push(w);
            }
            touched.push(touch(pi, None));
        }
        Edit::SetProposal { id, spec, notes, split } => {
            match (id, spec) {
                (None, None) => {
                    doc.pages.retain(|p| !proposal::is_proposal(p));
                    doc.datasets.retain(|d| !d.id.starts_with(proposal::DATASET));
                }
                (id, spec) => {
                    let id = id.unwrap_or_else(|| proposal::next_id(doc));
                    let (g, ds_id) = (proposal::group(&id), proposal::dataset(&id));
                    // in place of its old pages, or (a new one) after the others, before the legal text
                    let old_at = doc.pages.iter().position(|p| p.group == g);
                    doc.pages.retain(|p| p.group != g);
                    doc.datasets.retain(|d| d.id != ds_id);
                    if let Some(spec) = spec {
                        let ru = doc.theme.locale.starts_with("ru");
                        let (pages, ds, title) = proposal::build(&spec, &notes, ru, split, &id);
                        let at = old_at.unwrap_or_else(|| proposal::insert_at(doc));
                        let section = if ru { "Предложение" } else { "Proposal" };
                        for (k, (page_title, widgets)) in pages.into_iter().enumerate() {
                            let pid = doc.new_id("p");
                            let widgets = widgets.into_iter().map(|w| doc.instantiate(w)).collect();
                            doc.pages.insert(at + k, Page { id: pid, title: page_title, widgets, section: section.into(), group: g.clone(),
                                label: if k == 0 { section.into() } else { String::new() }, description: title.clone(), toc_hidden: k > 0, ..Default::default() });
                        }
                        doc.datasets.push(ds);
                        proposal::fit_views(doc);
                    }
                }
            }
        }
        Edit::AddPage { after } => {
            let pi = match after {
                Some(a) => doc.page_index(&a).ok_or("No such page.")?,
                None => doc.pages.len() - 1,
            };
            let n = doc.pages.len() + 1;
            new_page_after(doc, pi, format!("Page {n}"));
        }
        Edit::RemovePage { page } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            if doc.pages.len() == 1 {
                return Err("A report needs at least one page.".into());
            }
            doc.pages.remove(pi);
        }
        Edit::RenamePage { page, title } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            doc.pages[pi].title = title;
        }
        Edit::SetPageSection { page, section } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            doc.pages[pi].section = section.trim().to_string();
        }
        Edit::SetPagePlain { page, plain } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            doc.pages[pi].plain = plain;
        }
        Edit::SetPageRepeat { page, repeat, whole_group } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            if !doc.pages[pi].repeat_of.is_empty() {
                return Err("This page is a copy made for a repeated page; set the repeat on the page it copies.".into());
            }
            if !repeat.is_off() && doc.dataset(&repeat.dataset).is_none() {
                return Err("Choose a data source to repeat for.".into());
            }
            let group = doc.pages[pi].group.clone();
            for p in doc.pages.iter_mut().filter(|p| p.id == page || (whole_group && !group.is_empty() && p.group == group && !p.auto && p.repeat_of.is_empty())) {
                p.repeat = repeat.clone();
                if repeat.is_off() {
                    p.row.clear();
                    p.copy = 0;
                    p.copies = 0;
                }
            }
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::SetPageProps { page, group, label, frame, description, toc_hidden, show_if } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            let p = &mut doc.pages[pi];
            if let Some(v) = group {
                p.group = v.trim().to_string();
            }
            if let Some(v) = label {
                p.label = v.trim().to_string();
            }
            if let Some(v) = frame {
                p.frame = v;
            }
            if let Some(v) = description {
                p.description = v.trim().to_string();
            }
            if let Some(v) = show_if {
                p.show_if = v;
            }
            if let Some(v) = toc_hidden {
                p.toc_hidden = v;
            }
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::MovePage { page, to } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            let p = doc.pages.remove(pi);
            let to = to.min(doc.pages.len());
            doc.pages.insert(to, p);
        }
        Edit::DuplicatePage { page } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            let mut p = doc.pages[pi].clone();
            p.id = doc.new_id("p");
            p.title = format!("{} (copy)", p.title);
            for w in &mut p.widgets {
                w.id = doc.new_id("w");
            }
            doc.pages.insert(pi + 1, p);
        }
        Edit::SetFormats { formats } => {
            doc.formats = formats;
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::SetVariables { variables } => {
            doc.variables = variables;
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::SetIcons { icons } => {
            doc.icons = icons;
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::SetMeta { patch } => doc.meta = patched(&doc.meta, &patch)?,
        Edit::SetPageSetup { patch } => {
            doc.page = patched(&doc.page, &patch)?;
            theme_studio::bands_set_by_hand(&mut doc.page, &patch);
            doc.page.cols = doc.page.cols.clamp(1, 48);
            doc.page.rows = doc.page.rows.clamp(1, 64);
            doc.normalize();
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::SetTheme { patch } => {
            doc.theme = patched(&doc.theme, &patch)?;
            let roles = theme_studio::families_set(&doc.theme, &patch);
            if !roles.is_empty() {
                theme_studio::unpin_families(&mut doc.theme, &roles);
                touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
            }
            if doc.kind == DocKind::Theme {
                doc.meta.title = doc.theme.name.clone();
                theme_studio::rebuild(doc);
                touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
            }
        }
        Edit::SetPolicy { patch } => doc.policy = patched(&doc.policy, &patch)?,
        Edit::SetArrange { patch } => {
            doc.arrange = patched(&doc.arrange, &patch)?;
            if doc.arrange.auto {
                touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
            }
        }
        Edit::SetParams { params } => {
            doc.params = params;
            // tables may gain or lose rows with the new period
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::SetParamDefs { defs } => {
            for d in &defs {
                doc.params.values.entry(d.name.clone()).or_insert_with(|| d.default.clone());
            }
            doc.param_defs = defs;
        }
        Edit::UpsertDataset { mut dataset } => {
            if dataset.id.is_empty() {
                dataset.id = doc.new_id("d");
            }
            match doc.datasets.iter_mut().find(|d| d.id == dataset.id) {
                Some(d) => *d = dataset,
                None => doc.datasets.push(dataset),
            }
        }
        Edit::RemoveDataset { id } => doc.datasets.retain(|d| d.id != id),
        Edit::SetConnections { mut connections } => {
            for c in &mut connections {
                if c.id.is_empty() {
                    c.id = doc.new_id("c");
                }
            }
            doc.connections = connections;
        }
        Edit::UpsertCatalog { mut item } => {
            if item.id.is_empty() {
                item.id = doc.new_id("c");
            }
            match doc.catalog.iter_mut().find(|c| c.id == item.id) {
                Some(c) => *c = item,
                None => doc.catalog.push(item),
            }
        }
        Edit::RemoveCatalog { id } => doc.catalog.retain(|c| c.id != id),
        Edit::SaveToCatalog { id, name, category, description } => {
            let mut w = widget(doc, &id)?.clone();
            (w.id, w.x, w.y, w.locked, w.catalog) = (String::new(), 0, 0, false, None);
            let cid = doc.new_id("c");
            doc.catalog.push(CatalogItem { id: cid, name, description, category, widget: w, page: String::new() });
        }
        Edit::ApplyOps { page, ops } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            let mut pinned = None;
            let mut extra = Vec::new();
            for op in ops {
                match op {
                    Op::SetRect { id, rect } => {
                        if let Some((p, w)) = doc.find_widget(&id) {
                            let wd = &mut doc.pages[p].widgets[w];
                            (wd.x, wd.y, wd.w, wd.h) = (rect.x, rect.y, rect.w, rect.h);
                        }
                    }
                    Op::MoveToPage { id, page } => {
                        let (_, to, fresh) = move_to_page(doc, &id, page.as_deref())?;
                        extra.push(touch(to, if fresh { None } else { Some(id) }));
                    }
                    Op::Remove { id } => {
                        if let Some((p, w)) = doc.find_widget(&id) {
                            doc.pages[p].widgets.remove(w);
                        }
                    }
                    Op::SetLimit { id, n } => {
                        if let Some((p, w)) = doc.find_widget(&id) {
                            if let Some(b) = doc.pages[p].widgets[w].binding.as_mut() {
                                b.limit = n;
                                b.others = true;
                            }
                            pinned = Some(id);
                        }
                    }
                    Op::Paginate { id } => {
                        if let Some((p, w)) = doc.find_widget(&id) {
                            let wd = &mut doc.pages[p].widgets[w];
                            if !wd.opts.is_object() {
                                wd.opts = json!({});
                            }
                            let o = wd.opts.as_object_mut().unwrap();
                            let t = o.entry("table").or_insert_with(|| json!({}));
                            if let Some(t) = t.as_object_mut() {
                                t.insert("paginate".into(), json!(true));
                            }
                        }
                    }
                    Op::RemovePage { page } => {
                        if doc.pages.len() > 1 {
                            if let Some(i) = doc.page_index(&page) {
                                doc.pages.remove(i);
                            }
                        }
                    }
                }
            }
            // a suggestion is already a finished arrangement: only resolve overlaps
            let _ = pi;
            if let Some(pi) = doc.page_index(&page) {
                touched.push(Touch { page: pi, pinned, opts: Some(layout::Options::default()), firm: false });
            }
            touched.extend(extra);
        }
        Edit::AutoArrange { page } => {
            let pi = doc.page_index(&page).ok_or("No such page.")?;
            touched.push(Touch { page: pi, pinned: None, opts: Some(layout::Options { auto: true, fill: true }), firm: false });
        }
        Edit::AddFont { path } => {
            use base64::Engine as _;
            let bytes = std::fs::read(&path).map_err(|e| format!("Could not read the font: {e}"))?;
            if bytes.len() > 30_000_000 {
                return Err("That font file is too large to embed (over 30 MB).".into());
            }
            let names = fonts::add(bytes.clone())?;
            let file = Path::new(&path).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            doc.fonts.retain(|f| f.file != file);
            doc.fonts.push(model::EmbeddedFont {
                family: names[0].clone(),
                file,
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            });
            touched.extend((0..doc.pages.len()).map(|i| touch(i, None)));
        }
        Edit::RemoveFont { family } => doc.fonts.retain(|f| f.family != family),
    }
    Ok(touched)
}

/// A copy of a repeated page is rebuilt from the page it copies: what is added
/// to it goes on that page (and so on every copy).
fn designed_page(doc: &Doc, page: &str) -> String {
    doc.pages.iter().find(|p| p.id == page).map(|p| if p.repeat_of.is_empty() { p.id.clone() } else { p.repeat_of.clone() }).unwrap_or_else(|| page.to_string())
}

fn stamp_comment(w: &mut Widget, user: &str) {
    if !w.opts.is_object() {
        w.opts = json!({});
    }
    let o = w.opts.as_object_mut().unwrap();
    o.insert("author".into(), json!(user));
    o.insert("date".into(), json!(chrono::Local::now().format("%d.%m.%Y").to_string()));
}

/// Applies an edit with everything that follows it: ids, data-driven period
/// snapping, re-arranging touched pages and long-table continuations.
fn run_edit(app: &mut App, op: Edit) -> Result<Json, String> {
    // for the activity log: the change in one line, and the request itself
    let op_json = serde_json::to_value(&op).unwrap_or(Json::Null);
    let (kind, what) = audit::edit_line(&op_json, app.doc.as_ref());
    // what it changed: each value it sets, with the one it had (a widget's, a page's, the report's parameters)
    let changes = app.doc.as_ref().map(|d| {
        let before = if let Some(id) = op_json["id"].as_str() {
            d.find_widget(id).map(|(pi, wi)| serde_json::to_value(&d.pages[pi].widgets[wi]).unwrap_or(Json::Null))
        } else if let Some(pid) = op_json["page"].as_str().filter(|_| op_json["patch"].is_object()) {
            d.pages.iter().find(|p| p.id == pid).map(|p| serde_json::to_value(p).unwrap_or(Json::Null))
        } else if op_json["op"] == "set_params" {
            serde_json::to_value(&d.params).ok()
        } else {
            None
        };
        let patch = if op_json["op"] == "set_params" { &op_json["params"] } else { &op_json["patch"] };
        before.filter(|_| patch.is_object()).map(|b| audit::changes(&b, patch)).unwrap_or_default()
    }).unwrap_or_default();
    let summary: Vec<String> = changes.iter().take(3).map(|c| format!("{}: {} → {}", c["path"].as_str().unwrap_or("").trim_start_matches("values."), short_val(&c["from"]), short_val(&c["to"]))).collect();
    let what = match (what.is_empty(), summary.is_empty()) { (_, true) => what, (true, false) => summary.join("; "), _ => format!("{what} · {}", summary.join("; ")) };
    let r = run_edit_inner(app, op);
    app.audit("edit", &kind, "", &what, json!({"op": op_json, "changes": changes}), r.as_ref().err().map(|e| e.as_str()));
    r
}

fn run_edit_inner(app: &mut App, op: Edit) -> Result<Json, String> {
    let before = app.doc.clone();
    app.checkpoint();
    let snap = match &op {
        Edit::UpsertDataset { dataset } => Some(dataset.id.clone()),
        Edit::SetProposal { id: Some(id), spec: Some(_), .. } => Some(proposal::dataset(id)),
        _ => None,
    };
    match apply(app, op) {
        Ok(touched) => {
            if let Some(d) = app.doc.as_mut() {
                for p in &mut d.pages {
                    for w in &mut p.widgets {
                        w.ensure_ids();
                    }
                }
                for c in &mut d.catalog {
                    c.widget.ensure_ids();
                }
            }
            if let Some(id) = snap {
                app.refresh_fields_of(Some(&id));
                app.snap_period(&id);
            }
            for t in touched {
                app.arrange(t.page, t.pinned.as_deref(), t.opts, t.firm);
            }
            app.sync_chains();
            app.dirty = true;
            Ok(app.resp())
        }
        Err(e) => {
            app.undo.pop();
            app.doc = before;
            Err(e)
        }
    }
}

// ---------------------------------------------------------------- sign-in, roles and client access

/// The roles a command needs (any one of them); every command needs someone signed in.
fn gate(app: &App, roles: &[&str]) -> Result<(), String> {
    let s = app.session.as_ref().ok_or("Sign in first.")?;
    if roles.is_empty() || roles.iter().any(|r| s.has(r)) {
        return Ok(());
    }
    let names: Vec<String> = roles.iter().map(|r| { let mut c = r.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect();
    Err(format!("This needs the {} role.", names.join(" or ")))
}

/// The kinds a chart can switch between (the same data, drawn another way).
const CHART_KINDS: [&str; 9] = ["kpi", "table", "bar", "line", "area", "pie", "treemap", "tiles", "waterfall"];

/// Who may put a theme on the open document: a designer, or a report's user.
fn theme_roles(doc: Option<&Doc>) -> &'static [&'static str] {
    if doc.is_some_and(|d| d.kind == DocKind::Report) { &["design", "user"] } else { &["design"] }
}

/// Who may work on a document: templates and themes are Design work, reports are for users (and designers).
fn doc_roles(doc: Option<&Doc>) -> &'static [&'static str] {
    match doc.map(|d| &d.kind) {
        Some(DocKind::Report) => &["user", "design"],
        _ => &["design"],
    }
}

/// Opening a file: a template for design needs the Design role.
fn gate_open(app: &App, doc: &Doc) -> Result<(), String> {
    match gate(app, doc_roles(Some(doc))) {
        Err(_) if app.session.is_some() && doc.kind != DocKind::Report => Err("Templates open in the Design area. To make a report from one, use New report.".into()),
        r => r,
    }
}

/// A value of a client parameter that the signed-in person may use (every part of a multi-value).
fn client_ok(allowed: &[(String, String)], v: &str) -> bool {
    !v.is_empty() && v.split('\u{1f}').all(|x| allowed.iter().any(|(id, name)| id == x || name == x))
}

/// A report can't be switched to a client outside the person's access.
fn check_access_edit(app: &App, e: &Edit) -> Result<(), String> {
    let Some(doc) = &app.doc else { return Ok(()) };
    match e {
        Edit::SetParams { params } => {
            let Some(allowed) = &app.clients else { return Ok(()) };
            for d in doc.param_defs.iter().filter(|d| auth::is_client_param(d)) {
                if let Some(v) = params.values.get(&d.name) {
                    if Some(v) != doc.params.values.get(&d.name) && !client_ok(allowed, v) {
                        return Err(format!("“{}” isn't one of your clients.", v.replace('\u{1f}', ", ")));
                    }
                }
            }
            Ok(())
        }
        Edit::Batch { edits } => edits.iter().try_for_each(|e| check_access_edit(app, e)),
        // a shared source is changed in the Data area; a template only takes it as it is there
        Edit::UpsertDataset { dataset } if dataset.shared => {
            let st = app.store();
            let key = |d: &Dataset| serde_json::to_string(&(&d.name, &d.source, &d.date_field, d.snapshot, &d.transform, &d.calc_fields)).unwrap_or_default();
            match st.datasets.iter().find(|x| x.id == dataset.id) {
                Some(x) if key(x) == key(dataset) => Ok(()),
                _ => Err(format!("“{}” is a shared data source: change it in the Data area.", dataset.name)),
            }
        }
        // connections (and their credentials) come from the Data area: without the
        // Data role a template only takes shared ones or keeps those it has
        Edit::SetConnections { connections } if !app.session.as_ref().is_some_and(|s| s.has("data")) => {
            let shared = app.store().connections;
            let same = |a: &model::Connection, b: &model::Connection| a.id == b.id && a.kind == b.kind && a.path == b.path && a.url == b.url;
            let had = &doc.connections;
            match connections.iter().find(|c| !shared.iter().any(|x| same(x, c)) && !had.iter().any(|x| same(x, c))) {
                Some(c) => Err(format!("“{}”: connections are added and changed in the Data area (Data role). Pick one of the shared connections.", c.name)),
                None => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

impl App {
    /// Where the activity log lives: beside the accounts (the app's data folder).
    fn audit_dir(&self) -> Option<PathBuf> {
        self.store_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf())
    }

    /// Writes an event to the activity log for the person signed in (and the document in front
    /// when no target is given).
    fn audit(&self, area: &str, action: &str, target: &str, detail: &str, data: Json, error: Option<&str>) {
        // work on a document belongs to its session (sign-ins and administration don't)
        // administration (people, groups, reports' audiences, the bank's AI policy and keys) is the admin session's
        let admin_work = area == "admin" || (area == "ai" && matches!(action, "bank policy" | "save key" | "remove key") && self.session.as_ref().is_some_and(|s| s.has("admin")));
        let sid = if admin_work {
            self.admin_work_id()
        } else if matches!(area, "edit" | "document" | "data" | "ai") {
            self.work_id(matches!(area, "edit" | "ai") || matches!(action, "save" | "export" | "export brief"))
        } else {
            String::new()
        };
        self.audit_in(&sid, area, action, target, detail, data, error);
    }

    #[allow(clippy::too_many_arguments)]
    fn audit_in(&self, sid: &str, area: &str, action: &str, target: &str, detail: &str, data: Json, error: Option<&str>) {
        let user = self.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        let target = if target.is_empty() { self.doc.as_ref().map(|d| if d.kind == DocKind::Theme { d.theme.name.clone() } else { d.meta.title.clone() }).unwrap_or_default() } else { target.to_string() };
        audit::record_in(self.audit_dir().as_deref(), sid, &user, area, action, &target, detail, data, error);
    }

    /// The client a document is about (its client parameter's value), for its work session.
    fn doc_client(doc: &Doc) -> String {
        doc.param_defs.iter().filter(|d| auth::is_client_param(d))
            .filter_map(|d| doc.params.values.get(&d.name)).flat_map(|v| model::param_values(v).into_iter().filter(|x| *x != "All").map(String::from).collect::<Vec<_>>())
            .collect::<Vec<_>>().join(", ")
    }

    /// What a session is about: the document, the template it came from, the client, the period and the parameters.
    fn work_context(doc: &Doc) -> Json {
        json!({
            "kind": match doc.kind { DocKind::Report => "report", DocKind::Theme => "theme", _ => "template" },
            "title": doc.meta.title, "template": doc.meta.template_title, "report": doc.meta.report_id, "client": Self::doc_client(doc),
            "period": period::label(&doc.params), "params": doc.params.values,
            "from": period::period(&doc.params).start.to_string(), "to": period::period(&doc.params).end.to_string(),
        })
    }

    /// The work session of the document in front ("" with none): a new one when the document
    /// is new to it or its client changed. `busy`: something is done in it (not only looked at).
    /// A client picked before anything was done updates the session instead of starting another.
    fn work_id(&self, busy: bool) -> String {
        let Some(doc) = self.doc.as_ref().filter(|d| d.kind != DocKind::Theme) else { return String::new() };
        let client = Self::doc_client(doc);
        let user = self.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        let key = self.active_id;
        let (ended, started, context): (Option<Work>, Option<String>, bool) = {
            let mut w = self.work.borrow_mut();
            match w.open.get_mut(&key) {
                Some(cur) if cur.client == client && cur.user == user => {
                    cur.busy |= busy;
                    (None, None, false)
                }
                Some(cur) if !cur.busy && cur.user == user => {
                    cur.client = client.clone();
                    cur.busy |= busy;
                    (None, None, true)
                }
                _ => {
                    let old = w.open.remove(&key);
                    let id = format!("{}-{:x}", chrono::Local::now().format("%Y%m%d%H%M%S"), rand_u32());
                    w.open.insert(key, Work { id: id.clone(), client: client.clone(), user: user.clone(), active_s: 0.0, written_s: 0.0, busy, last: std::time::Instant::now(), seen: std::time::Instant::now() });
                    (old, Some(id), false)
                }
            }
        };
        if let Some(old) = ended {
            // the same document, now for another client (or another person signed in)
            self.end_work(old, "another client");
        }
        let id = self.work.borrow().open.get(&key).map(|w| w.id.clone()).unwrap_or_default();
        let ctx = Self::work_context(doc);
        let line = format!("{} · {}{}", doc.meta.title, if client.is_empty() { "без клиента".to_string() } else { client.clone() }, format!(" · {}", period::label(&doc.params)));
        if started.is_some() {
            self.audit_in(&id, "session", "start", &doc.meta.title, &line, ctx, None);
        } else if context {
            self.audit_in(&id, "session", "context", &doc.meta.title, &line, ctx, None);
        }
        id
    }

    /// The administrator's session (Admin area): one while they keep at it; 30 minutes away from
    /// it, or another person signed in, starts the next.
    fn admin_work_id(&self) -> String {
        let Some(s) = self.session.as_ref().filter(|s| s.has("admin")) else { return String::new() };
        let user = s.login.clone();
        let now = std::time::Instant::now();
        let stale = {
            let mut w = self.work.borrow_mut();
            match w.open.get_mut(&ADMIN_WORK) {
                Some(cur) if cur.user == user && now.duration_since(cur.seen).as_secs() < 30 * 60 => {
                    cur.seen = now;
                    return cur.id.clone();
                }
                _ => w.open.remove(&ADMIN_WORK),
            }
        };
        if let Some(old) = stale {
            let why = if old.user == user { "left" } else { "signed out" };
            self.end_work(old, why);
        }
        let id = format!("{}-{:x}", chrono::Local::now().format("%Y%m%d%H%M%S"), rand_u32());
        self.work.borrow_mut().open.insert(ADMIN_WORK, Work { id: id.clone(), client: String::new(), user: user.clone(), active_s: 0.0, written_s: 0.0, busy: true, last: now, seen: now });
        self.audit_in(&id, "session", "start", "Administration", "Administration", json!({"kind": "admin", "title": "Administration"}), None);
        id
    }

    /// Ends a session: its time and why it ended.
    fn end_work(&self, w: Work, reason: &str) {
        let line = format!("{} active", fmt_secs(w.active_s));
        // when it was last used (a session left idle ends later than that)
        let last = chrono::Local::now() - chrono::Duration::from_std(w.seen.elapsed()).unwrap_or_default();
        audit::record_in(self.audit_dir().as_deref(), &w.id, &w.user, "session", "end", "", &line, json!({"active_s": w.active_s.round(), "reason": reason, "last": last.to_rfc3339()}), None);
    }

    /// Ends the session of open document `key` (closed), or every session (signed out, quit).
    fn end_work_of(&self, key: Option<u64>, reason: &str) {
        let gone: Vec<Work> = {
            let mut w = self.work.borrow_mut();
            match key {
                Some(k) => w.open.remove(&k).into_iter().collect(),
                None => w.open.drain().map(|(_, v)| v).collect(),
            }
        };
        for g in gone {
            self.end_work(g, reason);
        }
    }

    /// Someone is working in the document in front: `secs` more of active time.
    fn work_time(&self, secs: f64, area: &str) {
        let (id, key) = if area == "admin" { (self.admin_work_id(), ADMIN_WORK) } else { (self.work_id(false), self.active_id) };
        if id.is_empty() {
            return;
        }
        let write = {
            let mut w = self.work.borrow_mut();
            let Some(cur) = w.open.get_mut(&key) else { return };
            let now = std::time::Instant::now();
            cur.seen = now;
            cur.active_s += secs.clamp(0.0, 120.0).min(now.duration_since(cur.last).as_secs_f64());
            cur.last = now;
            (cur.active_s - cur.written_s >= 300.0).then(|| {
                cur.written_s = cur.active_s;
                (cur.user.clone(), cur.active_s)
            })
        };
        if let Some((user, a)) = write {
            audit::record_in(self.audit_dir().as_deref(), &id, &user, "session", "time", "", "", json!({"active_s": a.round()}), None);
        }
    }

    fn store(&self) -> auth::Store {
        self.store_path.as_deref().map(auth::load).unwrap_or_else(auth::seed)
    }

    fn save_store(&self, s: &auth::Store) -> Result<(), String> {
        match &self.store_path {
            Some(p) => auth::save(p, s),
            None => Ok(()),
        }
    }

    fn start_session(&mut self, s: auth::Session) -> Result<(), String> {
        self.clients = auth::allowed(&self.store(), &s.access)?;
        // the Data area previews this year so far, for the first client this person may see
        let today = chrono::Local::now().date_naive();
        self.data_params = Params { grain: "custom".into(), start: chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&today), 1, 1), date: today, ..Default::default() };
        let first = match &self.clients {
            Some(c) => c.first().map(|x| x.1.clone()),
            None => auth::options(&self.store()).ok().and_then(|o| o["clients"].as_array().and_then(|a| a.first()).and_then(|c| c["name"].as_str().map(String::from))),
        };
        if let Some(c) = first {
            self.data_params.values.insert("client".into(), c);
        }
        self.engine.clear();
        self.user = s.name.clone();
        self.session = Some(s);
        Ok(())
    }

    fn end_session(&mut self) {
        self.data_report = None;
        self.engine.clear();
        self.session = None;
        self.clients = None;
        self.doc = None;
        self.path = None;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.parked.clear();
        self.active_link = None;
    }

    /// A client parameter set to a client outside the person's access moves
    /// to the first client they may see (or to none).
    fn enforce_access(&mut self) {
        let products = self.session.as_ref().map(|s| s.products()).unwrap_or_default();
        if let Some(d) = self.doc.as_mut() {
            d.access_src = products;
        }
        let Some(allowed) = self.clients.clone() else { return };
        let base = self.base_dir();
        let App { doc, engine, py, .. } = self;
        let Some(doc) = doc.as_mut() else { return };
        if doc.kind == DocKind::Theme {
            return;
        }
        let defs: Vec<ParamDef> = doc.param_defs.iter().filter(|d| auth::is_client_param(d)).cloned().collect();
        let mut opts: Option<Json> = None;
        let mut moved = false;
        for d in defs {
            let cur = doc.params.values.get(&d.name).cloned().unwrap_or_default();
            if client_ok(&allowed, &cur) {
                continue;
            }
            let o = opts.get_or_insert_with(|| engine.param_options(doc, base.as_deref(), py));
            let first = o
                .get(&d.name)
                .and_then(|v| v.as_array())
                .and_then(|a| a.iter().filter_map(|x| x.as_str()).find(|x| client_ok(&allowed, x)).map(String::from))
                .unwrap_or_else(|| allowed.first().map(|c| c.1.clone()).unwrap_or_default());
            doc.params.values.insert(d.name.clone(), first);
            moved = true;
        }
        // the moved client's own account pages
        if moved {
            self.sync_chains();
        }
    }

    /// The open document's shared sources take the Data area's current
    /// version (and their connections); one deleted there stays as the
    /// template's own copy. Then the template's use of them is recorded.
    fn sync_library(&mut self) {
        let mut st = self.store();
        let Some(doc) = self.doc.as_mut() else { return };
        if doc.kind == DocKind::Template && !doc.meta.report_id.is_empty() {
            let id = doc.meta.report_id.clone();
            if auth::apply_report_sources(&st, &id, doc) > 0 {
                self.notice = Some("This template has data sources added or changed in the Data area — they are in it now and go out with the next publish.".into());
            }
        }
        let mut gone = Vec::new();
        for d in doc.datasets.iter_mut().filter(|d| d.shared) {
            match st.datasets.iter().find(|x| x.id == d.id) {
                Some(x) => {
                    let fields = std::mem::take(&mut d.fields);
                    *d = x.clone();
                    d.shared = true;
                    if d.fields.is_empty() {
                        d.fields = fields;
                    }
                }
                None => {
                    d.shared = false;
                    gone.push(d.name.clone());
                }
            }
        }
        let need: Vec<String> = doc.datasets.iter().filter(|d| d.shared).filter_map(|d| match &d.source {
            model::Source::Db { connection, .. } => Some(connection.clone()),
            _ => None,
        }).collect();
        for id in need {
            if !doc.connections.iter().any(|c| c.id == id) {
                if let Some(c) = st.connections.iter().find(|c| c.id == id) {
                    doc.connections.push(c.clone());
                }
            }
        }
        if !gone.is_empty() {
            self.notice = Some(format!("{} left the Data area — this template keeps its own copy.", gone.join(", ")));
        }
        if self.record_usage(&mut st) {
            let _ = self.save_store(&st);
        }
    }

    /// Notes which shared sources the open (saved) template uses; true when it changed.
    fn record_usage(&self, st: &mut auth::Store) -> bool {
        let (Some(doc), Some(path)) = (&self.doc, &self.path) else { return false };
        if doc.kind != DocKind::Template {
            return false;
        }
        let path = path.display().to_string();
        let before = st.usage.clone();
        for v in st.usage.values_mut() {
            v.retain(|u| u.path != path);
        }
        for d in doc.datasets.iter().filter(|d| d.shared) {
            let widgets = doc.pages.iter().flat_map(|p| p.widgets.iter()).filter(|w| w.binding.as_ref().is_some_and(|b| b.dataset == d.id)).count();
            st.usage.entry(d.id.clone()).or_default().push(auth::UseRef { title: doc.meta.title.clone(), path: path.clone(), widgets });
        }
        st.usage.retain(|_, v| !v.is_empty());
        st.usage != before
    }

    fn session_json(&self) -> Json {
        match &self.session {
            Some(s) => json!({
                "login": s.login, "name": s.name, "roles": s.roles, "products": s.products(),
                "restricted": self.clients.is_some(),
                "clients": self.clients.as_ref().map(|c| c.len()),
            }),
            None => Json::Null,
        }
    }
}

/// Sign-in and the Admin and Data areas: one command with a sub-command
/// (`cmd`) and its arguments, shared by the app and the dev bridge.
/// The auth and administration commands, with what changes written to the activity log.
fn auth_cmd(app: &mut App, cmd: &str, args: &Json) -> Result<Json, String> {
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    // what the change was about, before it happens (a removed account or group is gone after)
    let before = app.store();
    let r = auth_cmd_inner(app, cmd, args);
    let err = r.as_ref().err().map(|e| e.as_str());
    match cmd {
        "login" => {
            // the person who tried, signed in or not
            let who = arg("login");
            let detail = if err.is_none() { "Signed in".to_string() } else { "Sign-in failed".to_string() };
            audit::record(app.audit_dir().as_deref(), &who, "auth", "sign-in", &who, &detail, Json::Null, err);
        }
        "logout" => {}
        "password" => app.audit("auth", "password", "", "Changed their password", Json::Null, err),
        "save_user" => {
            let a = &args["account"];
            let login = a["login"].as_str().unwrap_or("");
            let was = a["was"].as_str().unwrap_or("");
            let old = before.users.iter().find(|u| !was.is_empty() && u.login.eq_ignore_ascii_case(was)).map(|u| u.public());
            let mut changed: Vec<String> = Vec::new();
            if let Some(o) = &old {
                for k in ["name", "email", "roles", "access", "active", "must_change", "ai_level"] {
                    if o[k] != a[k] && !(a[k].is_null() && (o[k] == "" || o[k].is_null())) {
                        changed.push(k.replace('_', " "));
                    }
                }
                if !a["password"].as_str().unwrap_or("").is_empty() {
                    changed.push("password".into());
                }
            }
            let detail = if old.is_none() { format!("Added the account {login}") } else if changed.is_empty() { format!("Saved {login} (no changes)") } else { format!("Changed {login}: {}", changed.join(", ")) };
            let mut data = a.clone();
            if let Some(o) = data.as_object_mut() { o.remove("password"); }
            app.audit("admin", if old.is_none() { "add user" } else { "change user" }, login, &detail, json!({"after": data, "before": old}), err);
        }
        "delete_user" => app.audit("admin", "delete user", &arg("login"), &format!("Deleted the account {}", arg("login")), Json::Null, err),
        "save_group" => {
            let g = &args["group"];
            let name = g["name"].as_str().unwrap_or("");
            let n = g["members"].as_array().map(|m| m.len()).unwrap_or(0);
            let lvl = g["ai_level"].as_str().filter(|l| !l.is_empty()).map(|l| format!(" · AI {l}")).unwrap_or_default();
            app.audit("admin", if g["id"].as_str().unwrap_or("").is_empty() { "add group" } else { "change group" }, name, &format!("Group “{name}”: {n} member(s){lvl}"), g.clone(), err);
        }
        "delete_group" => {
            let name = before.groups.iter().find(|g| g.id == arg("id")).map(|g| g.name.clone()).unwrap_or_else(|| arg("id"));
            app.audit("admin", "delete group", &name, &format!("Deleted the group “{name}”"), Json::Null, err);
        }
        "set_user_groups" => app.audit("admin", "user groups", &arg("login"), &format!("{} is in: {}", arg("login"), args["groups"].as_array().map(|g| g.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default()), args["groups"].clone(), err),
        "save_connections" => app.audit("data", "connections", "", &format!("Saved {} connection(s)", args["connections"].as_array().map(|c| c.len()).unwrap_or(0)), Json::Null, err),
        "publish" => app.audit("admin", "publish", "", "Published the report", args.clone(), err),
        "set_audience" => app.audit("admin", "audience", &arg("id"), "Changed who gets the report", args["audience"].clone(), err),
        "api_save_client" => {
            let c = &args["client"];
            let was = c["was"].as_str().unwrap_or("");
            let name = c["name"].as_str().unwrap_or("");
            let detail = if was.is_empty() { format!("Added the API account “{name}” (a key made)") } else { format!("Changed the API account “{name}”") };
            app.audit("admin", if was.is_empty() { "add API account" } else { "change API account" }, name, &detail, c.clone(), err);
        }
        "api_new_key" => app.audit("admin", "API key", &arg("id"), &format!("Made a new key for the API account “{}” (the old one stopped working)", arg("id")), Json::Null, err),
        "api_delete_client" => app.audit("admin", "delete API account", &arg("id"), &format!("Deleted the API account “{}”", arg("id")), Json::Null, err),
        "api_save_report" => app.audit("admin", "API report", &arg("id"), &format!("Changed what “{}” accepts over the API", arg("id")), args["settings"].clone(), err),
        "api_save_target" => {
            let t = &args["target"];
            let mut data = t.clone();
            if let Some(o) = data.as_object_mut() { o.remove("secret_key"); }
            let name = t["name"].as_str().unwrap_or("");
            app.audit("admin", if t["was"].as_str().unwrap_or("").is_empty() { "add API destination" } else { "change API destination" }, name, &format!("API destination “{name}”"), data, err);
        }
        "api_delete_target" => app.audit("admin", "delete API destination", &arg("id"), &format!("Deleted the API destination “{}”", arg("id")), Json::Null, err),
        "api_test_target" => app.audit("admin", "test API destination", &arg("id"), &format!("Tested the API destination “{}”", arg("id")), Json::Null, err),
        "api_retention" => app.audit("admin", "API files", "", &format!("API files kept {} days", args["days"]), Json::Null, err),
        "save_report_source" | "save_source" | "delete_source" => app.audit("data", cmd, args["dataset"]["name"].as_str().unwrap_or(&arg("id")), &cmd.replace('_', " "), Json::Null, err),
        _ => {}
    }
    r
}

fn auth_cmd_inner(app: &mut App, cmd: &str, args: &Json) -> Result<Json, String> {
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let me = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
    let users = |st: &auth::Store| json!({ "users": st.users.iter().map(|a| a.public()).collect::<Vec<_>>(), "me": me });
    match cmd {
        "status" => {
            let st = app.store();
            // a demo convenience: accounts still on the demo password are offered on the sign-in screen
            let demo: Vec<Json> = st.users.iter().filter(|a| a.active && a.check(auth::DEMO_PASSWORD)).map(|a| json!({"login": a.login, "name": a.name, "roles": a.roles.effective()})).collect();
            Ok(json!({ "session": app.session_json(), "version": env!("CARGO_PKG_VERSION"), "demo": demo, "demo_password": if demo.is_empty() { "" } else { auth::DEMO_PASSWORD } }))
        }
        "login" => {
            let st = app.store();
            let login = arg("login");
            auth::sign_in_allowed(&login)?;
            let s = auth::sign_in(&st, &login, &arg("password")).inspect_err(|_| auth::sign_in_failed(&login))?;
            auth::sign_in_ok(&login);
            let must = st.users.iter().find(|a| a.login == s.login).map(|a| a.must_change).unwrap_or(false);
            app.end_session();
            app.start_session(s)?;
            let mut r = app.resp();
            r["must_change"] = json!(must);
            Ok(r)
        }
        "logout" => {
            app.end_work_of(None, "signed out");
            app.audit("auth", "sign-out", "", "Signed out", Json::Null, None);
            app.end_session();
            Ok(app.resp())
        }
        "password" => {
            gate(app, &[])?;
            let mut st = app.store();
            auth::change_password(&mut st, &me, &arg("old"), &arg("new"))?;
            app.save_store(&st)?;
            Ok(json!(true))
        }
        "users" => {
            gate(app, &["admin"])?;
            Ok(users(&app.store()))
        }
        "save_user" => {
            gate(app, &["admin"])?;
            let e: auth::AccountEdit = serde_json::from_value(args.get("account").cloned().unwrap_or(Json::Null)).map_err(|e| format!("Bad account: {e}"))?;
            let mut st = app.store();
            let own = e.was.eq_ignore_ascii_case(&me);
            auth::upsert(&mut st, e, &me)?;
            app.save_store(&st)?;
            if own {
                if let Some(a) = st.users.iter().find(|a| a.login.eq_ignore_ascii_case(&me)) {
                    let s = a.session();
                    app.start_session(s)?;
                }
            }
            Ok(users(&st))
        }
        "delete_user" => {
            gate(app, &["admin"])?;
            let mut st = app.store();
            auth::remove(&mut st, &arg("login"), &me)?;
            app.save_store(&st)?;
            // their AI keys go with them
            if !arg("login").eq_ignore_ascii_case(ai::org()) {
                for p in ai::PROVIDERS {
                    let _ = ai::key_set(p.id, &arg("login"), None);
                }
            }
            Ok(users(&st))
        }
        "options" => {
            gate(app, &["admin"])?;
            auth::options(&app.store())
        }
        // ---- the report API: the systems that ask for reports, and what each report accepts
        "api" => {
            gate(app, &["admin"])?;
            Ok(api_admin_view(app, &app.store()))
        }
        "api_save_client" => {
            gate(app, &["admin"])?;
            let e: api_accounts::ApiClientEdit = serde_json::from_value(args["client"].clone()).map_err(|e| format!("Bad API account: {e}"))?;
            let mut st = app.store();
            let key = api_accounts::save(&mut st, e, &me)?;
            app.save_store(&st)?;
            Ok(json!({"key": key, "api": api_admin_view(app, &st)}))
        }
        "api_new_key" => {
            gate(app, &["admin"])?;
            let mut st = app.store();
            let key = api_accounts::new_key(&mut st, &arg("id"))?;
            app.save_store(&st)?;
            Ok(json!({"key": key, "api": api_admin_view(app, &st)}))
        }
        "api_delete_client" => {
            gate(app, &["admin"])?;
            let mut st = app.store();
            api_accounts::remove(&mut st, &arg("id"))?;
            app.save_store(&st)?;
            Ok(api_admin_view(app, &st))
        }
        "api_save_report" => {
            gate(app, &["admin"])?;
            let id = arg("id");
            let r: api_accounts::ApiReport = serde_json::from_value(args["settings"].clone()).map_err(|e| format!("Bad settings: {e}"))?;
            let defs = report_doc(app, &id)?.param_defs;
            let mut st = app.store();
            api_accounts::set_report(&mut st, &id, r, &defs)?;
            app.save_store(&st)?;
            Ok(api_admin_view(app, &st))
        }
        "api_save_target" => {
            gate(app, &["admin"])?;
            let e: api_accounts::ApiTargetEdit = serde_json::from_value(args["target"].clone()).map_err(|e| format!("Bad destination: {e}"))?;
            let mut st = app.store();
            let id = api_accounts::save_target(&mut st, e, &me)?;
            app.save_store(&st)?;
            Ok(json!({"id": id, "api": api_admin_view(app, &st)}))
        }
        "api_delete_target" => {
            gate(app, &["admin"])?;
            let mut st = app.store();
            api_accounts::remove_target(&mut st, &arg("id"))?;
            app.save_store(&st)?;
            Ok(api_admin_view(app, &st))
        }
        "api_test_target" => {
            // writes a small file there (and takes it away from a folder): can Quire reach it?
            gate(app, &["admin"])?;
            let st = app.store();
            let t = st.api.targets.iter().find(|t| t.id == arg("id")).ok_or("No such destination.")?;
            Ok(json!({"at": api_delivery::test(t)?}))
        }
        "api_retention" => {
            gate(app, &["admin"])?;
            let days = args["days"].as_u64().filter(|d| (1..=365).contains(d)).ok_or("Keep files 1 to 365 days.")?;
            let mut st = app.store();
            st.api.retention_days = days as u32;
            app.save_store(&st)?;
            Ok(api_admin_view(app, &st))
        }
        "preview" => {
            gate(app, &["admin"])?;
            let a: auth::Access = serde_json::from_value(args.get("access").cloned().unwrap_or(Json::Null)).unwrap_or_default();
            let st = app.store();
            Ok(match auth::allowed(&st, &a)? {
                None => json!({ "all": true }),
                Some(c) => json!({ "all": false, "clients": c.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>() }),
            })
        }
        "my_clients" => {
            gate(app, &[])?;
            let st = app.store();
            let all = auth::options(&st).map(|o| o["clients"].clone()).unwrap_or(json!([]));
            let list: Vec<Json> = all.as_array().cloned().unwrap_or_default().into_iter().filter(|c| match &app.clients {
                None => true,
                Some(a) => a.iter().any(|(id, _)| Some(id.as_str()) == c["id"].as_str()),
            }).collect();
            Ok(json!(list))
        }
        // the same clients with something to read: figures, what's coming up, the person's own last reports
        "design_home" => {
            // the designer's start page: the published reports with how they're used, data changes
            // waiting to be taken in, and the template files opened lately
            gate(app, &["design"])?;
            let st = app.store();
            let login = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
            let since = (chrono::Local::now() - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
            let u = audit::design_usage(app.audit_dir().as_deref(), &login, &since);
            let reports: Vec<Json> = st.reports.iter().map(|r| {
                let mut j = serde_json::to_value(r).unwrap_or(Json::Null);
                j["reach"] = json!(st.reach(&r.audience).len());
                j["pending"] = json!(st.report_sources.get(&r.id).map(|l| l.iter().map(|d| d.name.clone()).collect::<Vec<_>>()).unwrap_or_default());
                j["usage"] = u["usage"].get(&r.id).or_else(|| u["usage"].get(&r.title)).cloned().unwrap_or(json!({"opened": 0, "exported": 0, "people": 0, "clients": 0, "last": ""}));
                // the theme its pages are drawn in (the themes view says which reports use a theme)
                j["theme"] = json!(published_pages(app, &r.id).map(|d| d.theme.name).unwrap_or_default());
                j
            }).collect();
            // every template: the library's files and the recent ones elsewhere, each with its published
            // report (and whether its pages changed since); reports published without a file after them
            let mut files: Vec<(PathBuf, String)> = templates_dir(app).and_then(|d| std::fs::read_dir(d).ok()).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "rtpl")).map(|p| (p, String::new())).collect()).unwrap_or_default();
            for f in u["recent"].as_array().into_iter().flatten() {
                if let (Some(p), Some(at)) = (f["path"].as_str(), f["at"].as_str()) {
                    match files.iter_mut().find(|(q, _)| q.as_path() == Path::new(p)) { Some(x) => x.1 = at.to_string(), None => files.push((PathBuf::from(p), at.to_string())) }
                }
            }
            let lib = templates_dir(app);
            let mut linked = std::collections::BTreeSet::new();
            let mut templates: Vec<Json> = files.into_iter().filter_map(|(p, at)| {
                let d = read_doc(&p).ok()?;
                let modified = std::fs::metadata(&p).ok().and_then(|m| m.modified().ok()).map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339()).unwrap_or_default();
                let rep = reports.iter().find(|r| !d.meta.report_id.is_empty() && r["id"] == d.meta.report_id).cloned();
                let changed = rep.as_ref().and_then(|r| r["id"].as_str()).and_then(|id| published_pages(app, id).ok()).map(|pd| changed_since_published(&d, &pd)).unwrap_or(false);
                if let Some(r) = &rep { linked.insert(r["id"].as_str().unwrap_or("").to_string()); }
                Some(json!({"path": p.display().to_string(), "title": d.meta.title, "theme": d.theme.name, "pages": d.pages.len(),
                    "in_library": lib.as_ref().is_some_and(|l| p.starts_with(l)), "opened": at, "modified": modified, "report": rep, "changed": changed}))
            }).collect();
            templates.sort_by(|a, b| b["modified"].as_str().unwrap_or("").cmp(a["modified"].as_str().unwrap_or("")));
            for r in &reports {
                if !linked.contains(r["id"].as_str().unwrap_or("")) {
                    templates.push(json!({"path": Json::Null, "title": r["title"], "theme": r["theme"], "report": r, "changed": false}));
                }
            }
            Ok(json!({"reports": reports, "templates": templates, "recent": u["recent"]}))
        }
        // ---- widget types: the designers' own, in a library every template can use
        "widget_types" => {
            gate(app, &["design"])?;
            let dir = widget_types_dir(app).ok_or("The widget types library is not available.")?;
            let doc = app.doc.as_ref();
            let list: Vec<Json> = list_widget_types(&dir).into_iter().map(|t| {
                let used = doc.map(|d| d.pages.iter().flat_map(|p| &p.widgets).filter(|w| w.wtype == t.id).count()).unwrap_or(0);
                let here = doc.and_then(|d| d.theme.widget_types.get(&t.id)).map(|x| x.version);
                let mut j = serde_json::to_value(&t).unwrap_or(Json::Null);
                j["used_here"] = json!(used);
                j["version_here"] = json!(here);
                j
            }).collect();
            Ok(json!(list))
        }
        "widget_type_save" => {
            // a widget's look saved as a type (new, or an update of the type it follows); it follows the type
            gate(app, &["design"])?;
            let dir = widget_types_dir(app).ok_or("The widget types library is not available.")?;
            let wid = args["widget"].as_str().ok_or("Which widget?")?.to_string();
            let doc = app.doc.as_ref().ok_or("Open a template first.")?;
            if doc.kind != DocKind::Template { return Err("Widget types are made in templates.".into()); }
            let (pi, wi) = doc.find_widget(&wid).ok_or("No such widget.")?;
            let w = doc.pages[pi].widgets[wi].clone();
            let prev = (!w.wtype.is_empty()).then(|| doc.theme.widget_types.get(&w.wtype).cloned()).flatten();
            let studio = !doc.widget_studio.is_empty();
            let update = args["update"].as_bool().unwrap_or(false) && prev.is_some();
            let name = args["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(String::from)
                .or_else(|| prev.as_ref().filter(|_| update).map(|p| p.name.clone())).ok_or("Give the type a name.")?;
            // its look: the type it follows (if any), then the widget's own changes
            let mut opts = prev.as_ref().map(|p| p.opts.clone()).filter(|o| o.is_object()).unwrap_or_else(|| json!({}));
            // (the widget studio's sample content is left out: it belongs to the sample data)
            let mut own_opts = w.opts.clone();
            if let (Some(o), Some(sample)) = (own_opts.as_object_mut(), doc.studio_sample.as_object()) {
                for (k, v) in sample { if o.get(k) == Some(v) { o.remove(k); } }
            }
            model::merge_json(&mut opts, &own_opts);
            let mut style = prev.as_ref().map(|p| p.style.clone()).filter(|o| o.is_object()).unwrap_or_else(|| json!({}));
            model::merge_json(&mut style, &w.style);
            let id = if update { prev.as_ref().unwrap().id.clone() } else {
                let base: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect::<String>().split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-");
                format!("{}-{:x}", if base.is_empty() { "widget".to_string() } else { base.chars().take(32).collect() }, rand_u32() & 0xffff)
            };
            let version = if update { read_widget_type(&dir, &id).map(|t| t.version).unwrap_or(prev.as_ref().unwrap().version) + 1 } else { 1 };
            let wt = model::WidgetType {
                id: id.clone(), name: name.clone(),
                description: args["description"].as_str().map(String::from).or_else(|| prev.as_ref().filter(|_| update).map(|p| p.description.clone())).unwrap_or_default(),
                icon: args["icon"].as_str().map(String::from).or_else(|| prev.as_ref().filter(|_| update).map(|p| p.icon.clone())).unwrap_or_default(),
                kind: w.kind.clone(),
                variant: if w.variant.is_empty() { prev.as_ref().map(|p| p.variant.clone()).unwrap_or_default() } else { w.variant.clone() },
                opts, style,
                // in the widget studio the data and title are samples: the type keeps its own
                binding: if studio { prev.as_ref().and_then(|p| p.binding.clone()) } else { w.binding.clone() },
                title: if studio { prev.as_ref().map(|p| p.title.clone()).unwrap_or_default() } else { w.title.clone() },
                subtitle: if studio { prev.as_ref().map(|p| p.subtitle.clone()).unwrap_or_default() } else { w.subtitle.clone() },
                w: if studio { prev.as_ref().map(|p| p.w).unwrap_or(4) } else { w.w }, h: if studio { prev.as_ref().map(|p| p.h).unwrap_or(3) } else { w.h }, version,
                by: app.session.as_ref().map(|s| s.name.clone()).unwrap_or_default(), updated: chrono::Local::now().to_rfc3339(),
                // a composite keeps its parts in the type (in the widget studio too: they are the design)
                parts: w.parts.clone(),
            };
            std::fs::write(dir.join(format!("{id}.json")), serde_json::to_string_pretty(&wt).map_err(|e| e.to_string())?).map_err(|e| format!("Could not save the type: {e}"))?;
            run_edit(app, Edit::AdoptWidgetType { id: wid, wt: wt.clone() })?;
            // in the widget studio: it edits this type from now on, and nothing is left unsaved
            if studio {
                if let Some(d) = app.doc.as_mut() { d.widget_studio = id.clone(); d.studio_sample = Json::Null; }
                app.dirty = false;
            }
            app.audit("design", if update { "widget type updated" } else { "widget type" }, &name, &format!("{} · version {version}", if update { "Updated the widget type" } else { "Saved a widget type" }), json!({"type": id}), None);
            Ok(json!({"state": app.resp(), "type": wt}))
        }
        "widget_type_delete" => {
            // out of the library; templates keep their copies (their widgets look the same)
            gate(app, &["design"])?;
            let dir = widget_types_dir(app).ok_or("The widget types library is not available.")?;
            let id = args["id"].as_str().filter(|i| !i.contains('/') && !i.contains("..")).ok_or("Which type?")?;
            let name = read_widget_type(&dir, id).map(|t| t.name).unwrap_or_default();
            std::fs::remove_file(dir.join(format!("{id}.json"))).map_err(|e| format!("Could not delete the type: {e}"))?;
            app.audit("design", "widget type deleted", &name, "Deleted a widget type", json!({"type": id}), None);
            Ok(Json::Null)
        }
        "widget_gallery" => {
            // the widgets library for the Design area's Widgets tab: every widget once — the built-in ones
            // (the design views, the same id in every theme that has it) and the designers' own — each drawn
            // on sample data, with the themes that include it and the templates that use it; drawn apart
            // from the open documents
            gate(app, &["design"])?;
            let theme_id = args["theme"].as_str().filter(|t| !t.is_empty()).unwrap_or("builtin:uhnwi");
            let (base, fonts) = load_theme(theme_id)?;
            // every theme: the built-in ones and the library's
            let mut all: Vec<(String, model::Theme)> = theme_studio::builtins().into_iter().filter_map(|(id, _)| theme_studio::builtin(id).map(|t| (format!("builtin:{id}"), t))).collect();
            if let Some(dir) = app.store_path.as_ref().and_then(|p| p.parent()).map(|d| d.join("themes")) {
                let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "qtheme")).collect()).unwrap_or_default();
                files.sort();
                for f in files { if let Ok((t, _)) = load_theme(&format!("file:{}", f.display())) { all.push((format!("file:{}", f.display()), t)); } }
            }
            // each built-in widget once: kind/view → its name, description and the themes that include it
            let mut seen: Vec<(String, String)> = Vec::new();
            let mut info: std::collections::BTreeMap<(String, String), (String, String, Vec<String>, model::StyleVariant)> = Default::default();
            for (_, t) in &all {
                for (kind, list) in &t.styles {
                    for v in list {
                        // the Quire theme's names for widgets the others share count as the same widget
                        let id = theme_studio::same_widget(kind, &v.id);
                        let key = (kind.clone(), id.to_string());
                        let e = info.entry(key.clone()).or_insert_with(|| { seen.push(key.clone()); (v.name.clone(), v.description.clone(), Vec::new(), v.clone()) });
                        // the shared widget's own view is the one drawn (not the Quire theme's copy)
                        if v.id == id && e.3.id != id { *e = (v.name.clone(), v.description.clone(), std::mem::take(&mut e.2), v.clone()); }
                        if !e.2.contains(&t.name) { e.2.push(t.name.clone()); }
                    }
                }
            }
            // names and descriptions in the preview theme's language: its own words where it has the widget
            let lang = base.locale.clone();
            for e in info.values_mut() { e.0 = theme_studio::tr(&lang, &e.0); e.1 = theme_studio::tr(&lang, &e.1); }
            for (kind, list) in &base.styles {
                for v in list {
                    if v.id != theme_studio::same_widget(kind, &v.id) { continue; }
                    if let Some(e) = info.get_mut(&(kind.clone(), v.id.clone())) { e.0 = v.name.clone(); e.1 = v.description.clone(); }
                }
            }
            // the preview theme gets the views it lacks, so every widget can be drawn in it
            let mut base = base;
            for ((kind, id), (_, _, _, v)) in &info {
                let l = base.styles.entry(kind.clone()).or_default();
                if !l.iter().any(|x| &x.id == id) { l.push(v.clone()); }
            }
            let mut doc = theme_studio::studio_doc(base, fonts);
            let lang = doc.theme.locale.clone();
            // where each widget is used: the library's templates and the published reports
            let mut used: std::collections::BTreeMap<String, Vec<String>> = Default::default();
            let mut tdocs: Vec<Doc> = templates_dir(app).and_then(|d| std::fs::read_dir(d).ok()).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "rtpl")).filter_map(|p| read_doc(&p).ok()).collect()).unwrap_or_default();
            for r in app.store().reports.iter() { if let Ok(d) = published_pages(app, &r.id) { tdocs.push(d); } }
            for d in &tdocs {
                let mut here = std::collections::BTreeSet::new();
                for w in d.pages.iter().flat_map(|p| &p.widgets) {
                    let key = if !w.wtype.is_empty() { format!("type:{}", w.wtype) } else {
                        let view = if w.variant.is_empty() { d.theme.styles.get(&w.kind).and_then(|l| l.first()).map(|v| v.id.clone()).unwrap_or_default() } else { w.variant.clone() };
                        format!("{}/{}", w.kind, theme_studio::same_widget(&w.kind, &view))
                    };
                    here.insert(key);
                }
                for k in here { let e = used.entry(k).or_default(); if !e.contains(&d.meta.title) { e.push(d.meta.title.clone()); } }
            }
            let mut items: Vec<Json> = Vec::new();
            for (pi, p) in doc.pages.iter().enumerate() {
                let Some(w) = p.widgets.first() else { continue };
                // a view the preview theme names its own way is listed once, under the shared widget
                if w.variant != theme_studio::same_widget(&w.kind, &w.variant) { continue; }
                let Some((name, desc, themes, _)) = info.get(&(w.kind.clone(), w.variant.clone())) else { continue };
                let key = format!("{}/{}", w.kind, w.variant);
                items.push(json!({"page": pi, "group": "builtin", "kind": w.kind, "view": w.variant, "name": name, "description": desc,
                    "themes": themes, "used": used.get(&key).cloned().unwrap_or_default()}));
            }
            // the designers' own, on sample data of their kind (their own data belongs to their templates)
            if let Some(dir) = widget_types_dir(app) {
                for t in list_widget_types(&dir) {
                    // on the sample its design view is shown with
                    let sample = doc.theme.variant(&t.kind, &t.variant).map(|x| x.sample.clone()).unwrap_or_default();
                    let (binding, content, title) = theme_studio::sample_widget(&t.kind, &sample, &lang);
                    let mut copy = t.clone();
                    let mut o = content;
                    if t.opts.is_object() { model::merge_json(&mut o, &t.opts); }
                    copy.opts = o;
                    doc.theme.widget_types.insert(t.id.clone(), copy);
                    let (w, h) = theme_studio::size(&t.kind);
                    let needs_data = !matches!(t.kind.as_str(), "heading" | "text" | "comment" | "divider" | "toc");
                    let widget = Widget { id: format!("wt_{}", t.id), kind: t.kind.clone(), w, h, wtype: t.id.clone(),
                        title: if t.title.is_empty() { title } else { t.title.clone() }, subtitle: t.subtitle.clone(),
                        binding: if needs_data { serde_json::from_value(binding).ok() } else { None }, ..Default::default() };
                    let page = model::Page { id: format!("p_wt_{}", t.id), title: t.name.clone(), widgets: vec![widget], ..Default::default() };
                    let base_view = info.get(&(t.kind.clone(), t.variant.clone())).map(|x| x.0.clone()).unwrap_or_default();
                    items.push(json!({"page": doc.pages.len(), "group": "mine", "kind": t.kind, "type": t.id, "name": t.name, "description": t.description,
                        "icon": t.icon, "version": t.version, "by": t.by, "updated": t.updated, "from": base_view,
                        "themes": Json::Null, "used": used.get(&format!("type:{}", t.id)).cloned().unwrap_or_default()}));
                    doc.pages.push(page);
                }
            }
            let _ = seen;
            let mut sub = batch_app(app);
            sub.load(doc, None);
            let st = sub.resp();
            Ok(json!({"state": st, "items": items, "themes": all.len()}))
        }
        "widget_open" => {
            // the widget studio: one widget on sample data with the full inspector, saved to the widget library —
            // a widget type of yours (`type`), or a built-in kind in a design view (`kind`, `view`) to make your own from.
            // The widget is the very one the Widgets tab previews, drawn larger.
            gate(app, &["design"])?;
            let theme_id = args["theme"].as_str().filter(|t| !t.is_empty()).unwrap_or("builtin:uhnwi");
            let (mut theme, fonts) = load_theme(theme_id)?;
            let mine = match args["type"].as_str() {
                Some(id) => Some(read_widget_type(&widget_types_dir(app).ok_or("The widget types library is not available.")?, id).ok_or("That widget type is no longer in the library.")?),
                None => None,
            };
            let (kind, view) = match &mine {
                Some(t) => (t.kind.clone(), t.variant.clone()),
                None => (args["kind"].as_str().ok_or("Which widget?")?.to_string(), args["view"].as_str().unwrap_or("").to_string()),
            };
            // a design view this theme lacks is borrowed from a theme that has it (the built-in widgets are the same everywhere)
            if !view.is_empty() && theme.variant(&kind, &view).is_none() {
                let found = theme_studio::builtins().into_iter().filter_map(|(id, _)| theme_studio::builtin(id)).find_map(|t| t.variant(&kind, &view).cloned());
                if let Some(v) = found { theme.styles.entry(kind.clone()).or_default().push(v); }
            }
            let mut doc = theme_studio::studio_doc(theme, fonts);
            let lang = doc.theme.locale.clone();
            doc.kind = DocKind::Template;
            // the gallery's widget for that view (its sample data, words and settings)
            let pick = doc.pages.iter().flat_map(|p| &p.widgets).find(|w| w.kind == kind && (view.is_empty() || w.variant == view)).cloned();
            let mut w = match (&mine, pick) {
                (_, Some(w)) => w,
                (_, None) => {
                    let (binding, content, title) = theme_studio::sample_widget(&kind, "", &lang);
                    Widget { kind: kind.clone(), variant: view.clone(), opts: content, title, binding: serde_json::from_value(binding).ok(), ..Default::default() }
                }
            };
            let name;
            if let Some(t) = &mine {
                // a type of yours: its own settings over the view; the sample's content stays under them
                let mut copy = t.clone();
                let mut o = w.opts.clone();
                if t.opts.is_object() { model::merge_json(&mut o, &t.opts); }
                copy.opts = o;
                if !t.title.is_empty() { w.title = t.title.clone(); }
                w.wtype = t.id.clone();
                w.variant = String::new();
                w.opts = json!({});
                name = t.name.clone();
                doc.theme.widget_types.insert(t.id.clone(), copy);
                doc.studio_sample = Json::Null;
            } else {
                name = doc.theme.variant(&kind, &view).map(|x| x.name.clone()).unwrap_or_else(|| kind.clone());
                // the preview carries its view's settings copied in: the view gives them here (nothing shows as
                // "set here" until changed); the sample's own content stays, and is left out when it is saved
                fn strip(c: &mut Json, v: &Json) {
                    if let (Some(co), Some(vo)) = (c.as_object_mut(), v.as_object()) {
                        for (k, vv) in vo {
                            match (co.get_mut(k), vv.is_object()) {
                                (Some(cv), true) if cv.is_object() => { strip(cv, vv); if cv.as_object().is_some_and(|m| m.is_empty()) { co.remove(k); } }
                                (Some(cv), _) if cv == vv => { co.remove(k); }
                                _ => {}
                            }
                        }
                    }
                }
                if let Some(v) = doc.theme.variant(&kind, &view).map(|v| v.opts.clone()) { strip(&mut w.opts, &v); }
                doc.studio_sample = w.opts.clone();
            }
            // drawn large in the middle of the page (the studio's fine grid)
            let (sw, sh) = theme_studio::size(&kind);
            let (sw, sh) = ((sw * 3 / 2).min(22), (sh * 3 / 2).min(20));
            w.id = "w_studio".into();
            w.x = (24 - sw) / 2;
            w.y = 2;
            w.w = sw;
            w.h = sh;
            doc.pages = vec![model::Page { id: "p_widget".into(), title: name.clone(), widgets: vec![w], ..Default::default() }];
            doc.meta.title = name;
            doc.widget_studio = mine.as_ref().map(|t| t.id.clone()).unwrap_or_else(|| "new".into());
            app.room()?;
            app.load(doc, None);
            app.dirty = false;
            Ok(app.resp())
        }
        "add_typed_widget" => {
            gate(app, &["design"])?;
            let dir = widget_types_dir(app).ok_or("The widget types library is not available.")?;
            let id = args["id"].as_str().ok_or("Which type?")?;
            let wt = read_widget_type(&dir, id).ok_or("That widget type is no longer in the library.")?;
            let page = args["page"].as_str().unwrap_or("").to_string();
            let at: Option<Rect> = args.get("at").and_then(|a| serde_json::from_value(a.clone()).ok());
            run_edit(app, Edit::AddTypedWidget { page, wt, at })?;
            Ok(app.resp())
        }
        "duplicate_template" => {
            // a copy of a template (a published report's, a file, or the example) under a new name, in
            // another theme: a new untitled template, to save and publish as its own
            gate(app, &["design"])?;
            let title = args["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or("Give the copy a name.")?.to_string();
            let mut d = if let Some(id) = args["report"].as_str() {
                published_doc(app, id)?
            } else if let Some(p) = args["path"].as_str() {
                read_doc(Path::new(p))?
            } else {
                match args["kind"].as_str() {
                    Some("uhnwi_report") => uhnwi_sample::template(),
                    _ => return Err("Which template should be copied?".into()),
                }
            };
            d.meta.title = title;
            d.meta.report_id = String::new();
            app.room()?;
            app.load(d, None);
            if let Some(t) = args["theme"].as_str().filter(|t| !t.is_empty()) {
                apply_theme_to(app, t)?;
                // the copy starts in its theme: nothing to undo back to
                app.undo.clear();
            }
            // kept in the templates library right away, so it shows in the Design area
            let path = save_template_here(app)?;
            app.audit("document", "save", &app.doc.as_ref().map(|d| d.meta.title.clone()).unwrap_or_default(), &path.display().to_string(), Json::Null, None);
            Ok(app.resp())
        }
        "edit_published" => {
            // a published report's template, opened to change it (publish again from its Access tab)
            gate(app, &["design"])?;
            let id = args["id"].as_str().ok_or("Which report?")?.to_string();
            let mut d = published_doc(app, &id)?;
            d.meta.report_id = id;
            app.room()?;
            app.load(d, None);
            Ok(app.resp())
        }
        "my_overview" => {
            gate(app, &[])?;
            let st = app.store();
            let ids: Vec<String> = match &app.clients {
                Some(a) => a.iter().map(|(id, _)| id.clone()).collect(),
                None => auth::options(&st).map(|o| o["clients"].as_array().cloned().unwrap_or_default().iter().filter_map(|c| c["id"].as_str().map(String::from)).collect()).unwrap_or_default(),
            };
            let path = auth::db_path(&st).ok_or("No bank database to read clients from.")?;
            let sources = app.session.as_ref().map(|s| s.products()).unwrap_or_default();
            let mut o = overview::overview_cached(&path, &ids, &sources, chrono::Local::now().date_naive())?;
            let login = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
            o["reports"] = audit::my_reports(app.audit_dir().as_deref(), &login);
            // the register: each client's proposals (newest first), and presented ones waiting for an outcome
            let names = allowed_names(app);
            let props = proposals::list(app.audit_dir().as_deref(), names.as_deref(), None);
            let mut by = serde_json::Map::new();
            for r in &props {
                let c = r["place"]["client"].as_str().unwrap_or("").to_string();
                let e = by.entry(c).or_insert_with(|| json!([]));
                e.as_array_mut().unwrap().push(json!({"uid": r["uid"], "title": r["content"]["title"], "status": r["status"], "created": r["created"], "updated": r["updated"],
                    "status_at": r["status_at"], "actions": r["content"]["actions"], "lead": r["content"]["lead"], "report": r["place"]["report"], "period": r["place"]["period"],
                    "in_report": r["in_report"], "history": r["history"], "notes": r["notes"], "banker": r["banker"]}));
            }
            o["proposals"] = Json::Object(by);
            let today = chrono::Local::now().date_naive();
            if let Some(att) = o["attention"].as_array_mut() {
                // the list groups items by client id
                let id_of: std::collections::HashMap<String, String> = match &app.clients {
                    Some(a) => a.iter().map(|(id, n)| (n.clone(), id.clone())).collect(),
                    None => auth::options(&st).map(|o| o["clients"].as_array().cloned().unwrap_or_default().iter()
                        .filter_map(|c| Some((c["name"].as_str()?.to_string(), c["id"].as_str()?.to_string()))).collect()).unwrap_or_default(),
                };
                for f in proposals::follow_ups(&props, today) {
                    let id = f["client"].as_str().and_then(|c| id_of.get(c)).cloned();
                    att.push(json!({"kind": "proposal", "client": f["client"], "id": id, "date": today.to_string(), "uid": f["uid"], "what": f["title"], "since": f["since"], "days": f["days"]}));
                }
            }
            Ok(o)
        }
        "connections" => {
            // designers pick from them; only the Data role changes them
            gate(app, &["data", "design"])?;
            // on the server the passwords stay there: the page sees them hidden
            let mut list = app.store().connections;
            if crate::secrets::enabled() {
                for c in &mut list {
                    if auth::split_password(&c.url).1.is_some() {
                        c.url = auth::with_password(&c.url, auth::HIDDEN);
                    }
                }
            }
            Ok(json!(list))
        }
        "save_connections" => {
            gate(app, &["data"])?;
            let mut list: Vec<model::Connection> = serde_json::from_value(args.get("connections").cloned().unwrap_or(json!([]))).map_err(|e| e.to_string())?;
            for (i, c) in list.iter_mut().enumerate() {
                if c.id.is_empty() {
                    c.id = format!("c{}{}", i, chrono::Utc::now().timestamp_millis() % 100_000);
                }
            }
            let mut st = app.store();
            // a password left hidden is the one kept
            for c in &mut list {
                *c = real_conn_in(&st, c.clone());
            }
            st.connections = list;
            app.save_store(&st)?;
            Ok(json!(st.connections))
        }
        // the Data area's client and period (used while no report is open)
        "data_params" => {
            gate(app, &["data", "design"])?;
            if let Some(v) = args.get("client").and_then(|v| v.as_str()) {
                if let Some(allowed) = &app.clients {
                    if !client_ok(allowed, v) {
                        return Err(format!("“{v}” isn't one of your clients."));
                    }
                }
                app.data_params.values.insert("client".into(), v.to_string());
            }
            let day = |k: &str| args.get(k).and_then(|v| v.as_str()).and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            if let (Some(a), Some(b)) = (day("start"), day("end")) {
                if a > b {
                    return Err("The period starts after it ends.".into());
                }
                app.data_params.grain = "custom".into();
                app.data_params.start = Some(a);
                app.data_params.date = b;
            }
            let r = period::period(&app.data_params);
            Ok(json!({
                "client": app.data_params.values.get("client").cloned().unwrap_or_default(),
                "start": r.start.format("%Y-%m-%d").to_string(), "end": r.end.format("%Y-%m-%d").to_string(),
            }))
        }
        // ---- groups (Admin), and who may use each published report
        "groups" => {
            gate(app, &["admin", "design"])?;
            let st = app.store();
            let people: Vec<Json> = st.users.iter().filter(|u| u.active).map(|u| json!({"login": u.login, "name": u.name, "user": u.roles.effective().user, "roles": u.roles.effective()})).collect();
            Ok(json!({ "groups": st.groups, "people": people }))
        }
        "save_group" => {
            gate(app, &["admin"])?;
            let mut g: auth::Group = serde_json::from_value(args.get("group").cloned().unwrap_or(Json::Null)).map_err(|e| format!("Bad group: {e}"))?;
            if g.name.trim().is_empty() {
                return Err("Give the group a name.".into());
            }
            let mut st = app.store();
            if g.id.is_empty() {
                let base: String = g.name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect::<String>().trim_matches('_').to_string();
                let base = if base.is_empty() { "group".to_string() } else { base };
                let mut id = base.clone();
                let mut n = 2;
                while st.groups.iter().any(|x| x.id == id) {
                    id = format!("{base}_{n}");
                    n += 1;
                }
                g.id = id;
            }
            g.members.sort();
            g.members.dedup();
            g.ai_level = auth::ai_level_ok(&g.ai_level)?;
            match st.groups.iter_mut().find(|x| x.id == g.id) {
                Some(x) => *x = g,
                None => st.groups.push(g),
            }
            app.save_store(&st)?;
            Ok(json!({ "groups": st.groups }))
        }
        "delete_group" => {
            gate(app, &["admin"])?;
            let id = arg("id");
            let mut st = app.store();
            st.groups.retain(|g| g.id != id);
            for r in &mut st.reports {
                r.audience.groups.retain(|g| g != &id);
            }
            app.save_store(&st)?;
            Ok(json!({ "groups": st.groups }))
        }
        // a person's groups, from their page in Admin
        "set_user_groups" => {
            gate(app, &["admin"])?;
            let login = arg("login");
            let ids: Vec<String> = serde_json::from_value(args.get("groups").cloned().unwrap_or(json!([]))).unwrap_or_default();
            let mut st = app.store();
            for g in &mut st.groups {
                g.members.retain(|m| !m.eq_ignore_ascii_case(&login));
                if ids.contains(&g.id) {
                    g.members.push(login.clone());
                    g.members.sort();
                }
            }
            app.save_store(&st)?;
            Ok(json!({ "groups": st.groups }))
        }
        // ---- published reports
        "reports" => {
            gate(app, &[])?;
            let st = app.store();
            let s = app.session.clone().ok_or("Sign in first.")?;
            let manage = s.has("admin") || s.has("design");
            // the Data role looks after every report's data (not who gets it)
            let every = manage || s.has("data");
            let list: Vec<Json> = st.reports.iter().filter(|r| every || st.may_use(r, &s)).map(|r| {
                let mut j = serde_json::to_value(r).unwrap_or(Json::Null);
                if manage {
                    j["reach"] = json!(st.reach(&r.audience));
                } else if let Some(o) = j.as_object_mut() {
                    o.remove("audience");
                }
                j
            }).collect();
            Ok(json!(list))
        }
        "publish" => {
            gate(app, &["design"])?;
            let me = app.session.as_ref().map(|s| s.name.clone()).unwrap_or_default();
            let mut st = app.store();
            let doc = app.doc.as_mut().ok_or("Open a template first.")?;
            if doc.kind != DocKind::Template {
                return Err("Only templates are published.".into());
            }
            if doc.meta.report_id.is_empty() {
                let base: String = doc.meta.title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect::<String>().split('_').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("_");
                let base = if base.is_empty() { "report".to_string() } else { base.chars().take(40).collect() };
                let mut id = base.clone();
                let mut n = 2;
                while st.reports.iter().any(|x| x.id == id) {
                    id = format!("{base}_{n}");
                    n += 1;
                }
                doc.meta.report_id = id;
                app.dirty = true;
            }
            // the Data area's changes for it are part of what goes out
            let id = app.doc.as_ref().unwrap().meta.report_id.clone();
            if auth::apply_report_sources(&st, &id, app.doc.as_mut().unwrap()) > 0 {
                app.dirty = true;
            }
            st.report_sources.remove(&id);
            let doc = app.doc.as_ref().unwrap();
            let dir = reports_dir(app).ok_or("No folder to publish to.")?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join(format!("{id}.rtpl")), serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let audience: Option<auth::Audience> = args.get("audience").and_then(|a| serde_json::from_value(a.clone()).ok());
            let entry = match st.reports.iter_mut().find(|r| r.id == id) {
                Some(r) => r,
                None => {
                    st.reports.push(auth::Published { id: id.clone(), audience: auth::Audience { everyone: true, ..Default::default() }, ..Default::default() });
                    st.reports.last_mut().unwrap()
                }
            };
            entry.title = doc.meta.title.clone();
            entry.description = doc.meta.description.clone();
            entry.published = true;
            entry.by = me;
            entry.at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
            entry.version += 1;
            if let Some(a) = audience {
                entry.audience = a;
            }
            let out = entry.clone();
            app.save_store(&st)?;
            // the template keeps its link to the report: saved where it is, or into the library
            let saved = save_template_here(app).ok().map(|p| p.display().to_string());
            Ok(json!({ "report": out, "reach": st.reach(&out.audience), "state": app.resp(), "saved": saved }))
        }
        // who may use it, or a draft — from the template's Access tab (Design) or a person's page (Admin)
        "set_audience" => {
            gate(app, &["design", "admin"])?;
            let id = arg("id");
            let mut st = app.store();
            let r = st.reports.iter_mut().find(|r| r.id == id).ok_or("That report isn't published yet.")?;
            if let Some(a) = args.get("audience").and_then(|a| serde_json::from_value::<auth::Audience>(a.clone()).ok()) {
                r.audience = a;
            }
            if let Some(p) = args.get("published").and_then(|v| v.as_bool()) {
                r.published = p;
            }
            let out = r.clone();
            app.save_store(&st)?;
            Ok(json!({ "report": out, "reach": st.reach(&out.audience) }))
        }
        // a template no longer needed: its file and, when it was published, its report (report users lose it).
        // Nothing is erased: both go to a ".deleted" folder in the templates library
        "delete_template" => {
            gate(app, &["design"])?;
            let path = arg("path");
            let rid = arg("report");
            let lib = templates_dir(app).ok_or("The templates library is not available.")?;
            let bin = lib.join(".deleted");
            std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
            let put_away = |from: &Path| -> Result<(), String> {
                let name = from.file_stem().and_then(|n| n.to_str()).unwrap_or("template");
                let to = bin.join(format!("{name} · {stamp}.rtpl"));
                if std::fs::rename(from, &to).is_err() {
                    std::fs::copy(from, &to).map_err(|e| e.to_string())?;
                    std::fs::remove_file(from).map_err(|e| e.to_string())?;
                }
                Ok(())
            };
            let mut gone = Vec::new();
            if !path.is_empty() {
                let p = PathBuf::from(&path);
                if p.extension().is_none_or(|e| e != "rtpl") || !p.is_file() {
                    return Err("That template file isn't there any more.".into());
                }
                put_away(&p)?;
                gone.push("template");
            }
            if !rid.is_empty() {
                let mut st = app.store();
                let before = st.reports.len();
                st.reports.retain(|r| r.id != rid);
                st.report_sources.remove(&rid);
                if st.reports.len() != before {
                    app.save_store(&st)?;
                    if let Some(f) = reports_dir(app).map(|d| d.join(format!("{rid}.rtpl"))).filter(|f| f.is_file()) {
                        put_away(&f)?;
                    }
                    gone.push("report");
                }
            }
            app.audit("design", "delete_template", &path, &format!("Deleted: {}", gone.join(" and ")), json!({"path": path, "report": rid}), None);
            Ok(json!({ "deleted": gone, "bin": bin.display().to_string() }))
        }
        // a published report's own data, for the Data area (read-only)
        "report_data" => {
            gate(app, &["data", "design"])?;
            let id = arg("id");
            if id.is_empty() {
                app.data_report = None;
                return Ok(Json::Null);
            }
            let d = published_doc(app, &id)?;
            if !args.get("peek").and_then(|v| v.as_bool()).unwrap_or(false) {
                app.data_report = Some(id.clone());
            }
            let st = app.store();
            let title = st.reports.iter().find(|r| r.id == id).map(|r| r.title.clone()).unwrap_or_default();
            let changed: Vec<String> = st.report_sources.get(&id).map(|l| l.iter().map(|d| d.id.clone()).collect()).unwrap_or_default();
            // added here (not in the published pages) vs changed here
            let base: Vec<String> = published_pages(app, &id).map(|b| b.datasets.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
            let added: Vec<String> = changed.iter().filter(|c| !base.contains(c)).cloned().collect();
            Ok(json!({ "id": id, "title": title, "datasets": d.datasets, "connections": d.connections, "param_defs": d.param_defs, "changed": changed, "added": added }))
        }
        // a data source added to or changed in a published report, from the Data area
        "save_report_source" => {
            gate(app, &["data", "design"])?;
            let id = arg("id");
            let mut d: Dataset = serde_json::from_value(args.get("dataset").cloned().unwrap_or(Json::Null)).map_err(|e| format!("Bad data source: {e}"))?;
            if d.name.trim().is_empty() {
                return Err("Give the data source a name.".into());
            }
            let base = published_doc(app, &id)?;
            if d.id.is_empty() {
                let mut n = base.datasets.len() + 1;
                while base.datasets.iter().any(|x| x.id == format!("dx{n}")) {
                    n += 1;
                }
                d.id = format!("dx{n}");
            }
            d.shared = false;
            let mut st = app.store();
            let list = st.report_sources.entry(id.clone()).or_default();
            match list.iter_mut().find(|x| x.id == d.id) {
                Some(x) => *x = d.clone(),
                None => list.push(d.clone()),
            }
            app.save_store(&st)?;
            app.data_report = Some(id);
            Ok(json!({ "id": d.id }))
        }
        // undo the Data area's change to a report source (an added one goes)
        "drop_report_source" => {
            gate(app, &["data", "design"])?;
            let (id, ds) = (arg("id"), arg("dataset"));
            let mut st = app.store();
            if let Some(l) = st.report_sources.get_mut(&id) {
                l.retain(|x| x.id != ds);
            }
            st.report_sources.retain(|_, l| !l.is_empty());
            app.save_store(&st)?;
            Ok(json!(true))
        }
        // which sources follow each report parameter (the Data area's parameter cards)
        "param_reach" => {
            gate(app, &["data", "design"])?;
            let doc = preview_doc(app);
            Ok(json!(doc.param_defs.iter().map(|d| query::param_reach(&doc, &d.name)).collect::<Vec<_>>()))
        }
        "library" => {
            gate(app, &["data", "design"])?;
            let st = app.store();
            Ok(json!({ "datasets": st.datasets, "usage": st.usage }))
        }
        "save_source" => {
            gate(app, &["data"])?;
            let mut d: Dataset = serde_json::from_value(args.get("dataset").cloned().unwrap_or(Json::Null)).map_err(|e| format!("Bad data source: {e}"))?;
            if d.name.trim().is_empty() {
                return Err("Give the data source a name.".into());
            }
            let mut st = app.store();
            if d.id.is_empty() || !st.datasets.iter().any(|x| x.id == d.id) {
                let base: String = d.name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>().trim_matches('_').to_string();
                let base = format!("s_{}", if base.is_empty() { "source".to_string() } else { base });
                let mut id = base.clone();
                let mut n = 2;
                while st.datasets.iter().any(|x| x.id == id) {
                    id = format!("{base}_{n}");
                    n += 1;
                }
                d.id = id;
            }
            d.shared = true;
            match st.datasets.iter_mut().find(|x| x.id == d.id) {
                Some(x) => *x = d.clone(),
                None => st.datasets.push(d.clone()),
            }
            app.save_store(&st)?;
            // the open template's copy follows at once
            let had = app.doc.as_ref().is_some_and(|doc| doc.datasets.iter().any(|x| x.id == d.id && x.shared));
            app.sync_library();
            if had {
                app.sync_chains();
            }
            Ok(json!({ "id": d.id, "state": app.resp() }))
        }
        "delete_source" => {
            gate(app, &["data"])?;
            let id = arg("id");
            let mut st = app.store();
            st.datasets.retain(|x| x.id != id);
            st.usage.remove(&id);
            app.save_store(&st)?;
            app.sync_library();
            Ok(json!({ "state": app.resp() }))
        }
        "use_source" => {
            gate(app, &["design"])?;
            let st = app.store();
            let id = arg("id");
            let d = st.datasets.iter().find(|x| x.id == id).cloned().ok_or("That data source is no longer in the Data area.")?;
            let doc = app.doc.as_ref().ok_or("Open a template first.")?;
            if doc.kind != DocKind::Template {
                return Err("Shared data sources are added to templates.".into());
            }
            let mut edits = Vec::new();
            if let model::Source::Db { connection, .. } = &d.source {
                if !doc.connections.iter().any(|c| &c.id == connection) {
                    let c = st.connections.iter().find(|c| &c.id == connection).cloned().ok_or("Its connection is no longer in the Data area.")?;
                    let mut all = doc.connections.clone();
                    all.push(c);
                    edits.push(Edit::SetConnections { connections: all });
                }
            }
            edits.push(Edit::UpsertDataset { dataset: d });
            let op = Edit::Batch { edits };
            check_access_edit(app, &op)?;
            run_edit(app, op)
        }
        _ => Err(format!("Unknown request “{cmd}”.")),
    }
}

/// A colour picked anywhere on screen (another app's window included) with the system
/// eyedropper, as "#rrggbb"; None when the user cancels.
fn pick_screen_color(state: St<'_>) -> Result<Option<String>, String> {
    state.host().pick_color()
}

fn auth(state: St<'_>, cmd: String, args: Option<Json>) -> Result<Json, String> {
    let mut app = lock(&state);
    let was = app.session.as_ref().map(|s| s.login.clone());
    let r = auth_cmd(&mut app, &cmd, &args.unwrap_or(Json::Null));
    // someone else in front (signed in, out, or another person): their settings, their Python
    let now = app.session.as_ref().map(|s| s.login.clone());
    if now != was {
        state.host().signed_in(now.as_deref());
        let s = read_settings(settings_path(state.host()).as_deref());
        app.py.set_python(Some(s.python.clone()));
        app.settings = s;
        app.engine.clear();
    }
    r
}

fn edit(state: St<'_>, op: Edit) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, doc_roles(app.doc.as_ref()))?;
    check_access_edit(&app, &op)?;
    run_edit(&mut app, op)
}

/// `undo`, recorded in the activity log.
fn undo(state: St<'_>) -> Result<Json, String> {
    
    let r = undo_run(state);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("edit", "undo", "", "", Json::Null, err);
    }
    r
}

fn undo_run(state: St<'_>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, doc_roles(app.doc.as_ref()))?;
    if let Some(prev) = app.undo.pop() {
        if let Some(cur) = app.doc.replace(prev) {
            app.redo.push(cur);
        }
        app.dirty = true;
    }
    Ok(app.resp())
}

/// `redo`, recorded in the activity log.
fn redo(state: St<'_>) -> Result<Json, String> {
    
    let r = redo_run(state);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("edit", "redo", "", "", Json::Null, err);
    }
    r
}

fn redo_run(state: St<'_>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, doc_roles(app.doc.as_ref()))?;
    if let Some(next) = app.redo.pop() {
        if let Some(cur) = app.doc.replace(next) {
            app.undo.push(cur);
        }
        app.dirty = true;
    }
    Ok(app.resp())
}

fn get_state(state: St<'_>) -> Result<Json, String> {
    Ok(lock(&state).resp())
}

fn refresh(state: St<'_>) -> Result<Json, String> {
    let mut app = lock(&state);
    app.engine.clear();
    app.py.clear_cache();
    let n = app.doc.as_ref().map(|d| d.pages.len()).unwrap_or(0);
    for pi in 0..n {
        app.arrange(pi, None, None, false);
    }
    app.sync_chains();
    Ok(app.resp())
}

// ------------------------------------------------------------------ files

/// A variable's value in its format: number, currency, percent or date.
fn format_var(raw: &str, format: &str, doc: &Doc) -> String {
    let num = data::parse_num(raw);
    match (format, num) {
        ("number", Some(n)) => data::fmt_num_locale(n),
        ("currency", Some(n)) => {
            let f = period::formats();
            let cur = if f.currency.is_empty() { doc.theme.currency.clone() } else { f.currency.clone() };
            let after = if f.currency_pos.is_empty() { !doc.theme.locale.starts_with("en") } else { f.currency_pos == "after" };
            if after { format!("{}\u{a0}{cur}", data::fmt_num_locale(n)) } else { format!("{cur}{}", data::fmt_num_locale(n)) }
        }
        ("percent", Some(n)) => {
            let p = (n * 1000.0).round() / 10.0;
            // one decimal at most: 25.6%, 26%
            let mut s = data::fmt_num_locale(p);
            if p.fract() != 0.0 && s.ends_with('0') {
                s.pop();
            }
            format!("{s}%")
        }
        ("date", _) => data::parse_date(raw).map(period::fmt_date).unwrap_or_else(|| raw.to_string()),
        _ => raw.to_string(),
    }
}

/// Every placeholder a text can use, with how it is filled and its value now.
fn variables_list(doc: &Doc) -> Json {
    let x = |t: &str| vm::expand(t, doc);
    let mut out = vec![
        json!({"group": "Period", "token": "{{period}}", "label": "Reporting period", "how": "The period picked in the toolbar, written out (e.g. “June 2026”).", "value": x("{{period}}")}),
        json!({"group": "Period", "token": "{{start}}", "label": "Period start", "how": "First day of the period, in the document’s date format.", "value": x("{{start}}")}),
        json!({"group": "Period", "token": "{{end}}", "label": "Period end", "how": "Last day of the period (the “as of” date).", "value": x("{{end}}")}),
        json!({"group": "Period", "token": "{{compare}}", "label": "Comparison period", "how": "The period compared with: previous period or same period last year.", "value": x("{{compare}}")}),
        json!({"group": "Period", "token": "{{today}}", "label": "Today", "how": "The date the report is opened or exported.", "value": x("{{today}}")}),
        json!({"group": "Document", "token": "{{title}}", "label": "Title", "how": "Document → Title.", "value": x("{{title}}")}),
        json!({"group": "Document", "token": "{{subtitle}}", "label": "Subtitle", "how": "Document → Subtitle.", "value": x("{{subtitle}}")}),
        json!({"group": "Document", "token": "{{organization}}", "label": "Organization", "how": "Document → Organization.", "value": x("{{organization}}")}),
        json!({"group": "Document", "token": "{{author}}", "label": "Author", "how": "Document → Author (the report user in reports).", "value": x("{{author}}")}),
        json!({"group": "Page", "token": "{{page}}", "label": "Page number", "how": "The page’s number. In headers and footers only.", "value": ""}),
        json!({"group": "Page", "token": "{{pages}}", "label": "Page count", "how": "How many pages the report has. In headers and footers only.", "value": doc.pages.len().to_string()}),
        json!({"group": "Page", "token": "{{section}}", "label": "Section", "how": "The page’s section. In headers and footers only.", "value": ""}),
    ];
    for p in &doc.param_defs {
        let t = format!("{{{{{}}}}}", p.name);
        out.push(json!({"group": "Parameters", "token": t, "label": if p.label.is_empty() { &p.name } else { &p.label }, "how": "The value the report user picks in the toolbar (several are joined with commas). In SQL: :".to_string() + &p.name + ".", "value": x(&t)}));
    }
    for v in doc.variables.iter().filter(|v| !v.name.trim().is_empty()) {
        let t = format!("{{{{{}}}}}", v.name.trim());
        let how = match v.kind.as_str() {
            "sql" => format!("SQL: {}", v.value.trim()),
            "data" => format!("Data value: {}", v.value.trim()),
            _ => format!("Text: {}", v.value),
        };
        out.push(json!({"group": "My variables", "token": t, "label": if v.label.is_empty() { &v.name } else { &v.label }, "how": how, "value": x(&t), "custom": v.name.trim(), "description": v.description}));
    }
    json!(out)
}

// ------------------------------------------------------------------ app settings

/// Preferences for this computer: formats and defaults new documents start with.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    /// The interface language: "en" (default) or "ru".
    ui_lang: String,
    formats: model::Formats,
    /// Language of new documents ("" = the theme's)
    locale: String,
    /// Currency symbol of new documents
    currency: String,
    author: String,
    organization: String,
    /// "A4", "A3", "Letter", "Slide"
    page_size: String,
    landscape: Option<bool>,
    /// Period grain and comparison new documents open with
    grain: String,
    compare: String,
    /// The Python interpreter for Python sources and the SQL workspace ("" = find one).
    python: String,
    /// AI assistants: the provider and each provider's model and endpoint (keys are in the Keychain).
    ai: ai::AiSettings,
}

/// The settings file of the person in front (the computer's on the desktop; each person's on the server).
fn settings_path(host: &dyn Host) -> Option<PathBuf> {
    let p = host.settings_file()?;
    std::fs::create_dir_all(p.parent()?).ok()?;
    Some(p)
}

fn read_settings(path: Option<&Path>) -> Settings {
    path.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// A new document takes the settings: formats, language, money, author, page, period.
fn apply_settings(doc: &mut Doc, s: &Settings) {
    doc.formats = s.formats.clone();
    if !s.locale.is_empty() {
        doc.theme.locale = s.locale.clone();
    }
    if !s.currency.is_empty() {
        doc.theme.currency = s.currency.clone();
    }
    if !s.author.is_empty() {
        doc.meta.author = s.author.clone();
    }
    if !s.organization.is_empty() {
        doc.meta.organization = s.organization.clone();
    }
    if !s.page_size.is_empty() {
        doc.page.size = s.page_size.clone();
    }
    if let Some(l) = s.landscape {
        doc.page.landscape = l;
    }
    if !s.grain.is_empty() {
        doc.params.grain = s.grain.clone();
    }
    if !s.compare.is_empty() {
        doc.params.compare = s.compare.clone();
    }
}

fn get_settings(state: St<'_>) -> Result<Settings, String> {
    Ok(read_settings(settings_path(state.host()).as_deref()))
}

/// Saves the settings; open documents that don't set their own formats follow them at once.
fn save_settings(state: St<'_>, settings: Settings) -> Result<Json, String> {
    let app_handle: H = state.host();
    let p = settings_path(app_handle).ok_or("No settings folder.")?;
    std::fs::write(p, serde_json::to_string_pretty(&settings).unwrap_or_default()).map_err(|e| e.to_string())?;
    let mut app = lock(&state);
    let py = settings.python.clone();
    app.py.set_python(Some(py));
    app.settings = settings;
    app.engine.clear();
    Ok(app.resp())
}

/// Opens the web inspector (a developer option: right-click with ⌥ held).
fn open_devtools(state: St<'_>) -> Result<(), String> {
    state.host().devtools();
    Ok(())
}

/// `new_doc`, recorded in the activity log.
fn new_doc(state: St<'_>, kind: String, client: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let k = kind.clone();
    let r = new_doc_run(app_handle, state, kind, client);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("document", "new", "", &format!("New document ({k})"), Json::Null, err);
    }
    r
}

/// A report made for a client is made for that client from the start (not for the template's
/// default client first: that would count as opening a report for them).
fn for_client(mut d: Doc, client: Option<&str>) -> Doc {
    if let Some(c) = client.filter(|c| !c.is_empty()) {
        let names: Vec<String> = d.param_defs.iter().filter(|p| auth::is_client_param(p)).map(|p| p.name.clone()).collect();
        for n in names {
            d.params.values.insert(n, c.to_string());
        }
    }
    d
}

fn new_doc_run(app_handle: H, state: St<'_>, kind: String, client: Option<String>) -> Result<Json, String> {
    let client = client.as_deref();
    let settings = read_settings(settings_path(app_handle).as_deref());
    let mut app = lock(&state);
    app.room()?;
    // the examples as end users get them are reports; everything else starts a template
    if let Some(id) = kind.strip_prefix("pub:") {
        let d = report_from_published(&app, id)?;
        app.load(for_client(d, client), None);
        app.dirty = true;
        return Ok(app.resp());
    }
    let roles: &[&str] = if matches!(kind.as_str(), "sample_report" | "uhnwi_as_report") { &["user", "design"] } else { &["design"] };
    gate(&app, roles)?;
    if kind == "uhnwi_as_report" && !app.session.as_ref().is_some_and(|s| s.has("design")) {
        let d = report_from_published(&app, "uhnwi")?;
        app.load(for_client(d, client), None);
        app.dirty = true;
        return Ok(app.resp());
    }
    let doc = match kind.as_str() {
        "sample" => sample::template(),
        "sample_report" => {
            let mut d = sample::template();
            to_report(&mut d, None);
            d
        }
        "client_report" => client_sample::template(),
        "uhnwi_report" => uhnwi_sample::template(),
        // the UHNWI example as end users get it
        "uhnwi_as_report" => {
            let mut d = uhnwi_sample::template();
            to_report(&mut d, None);
            d
        }
        _ => {
            // a blank document starts from this computer's settings
            let mut d = sample::blank();
            apply_settings(&mut d, &settings);
            d
        }
    };
    app.load(for_client(doc, client), None);
    app.dirty = kind != "sample";
    Ok(app.resp())
}

/// Puts the widgets linked to each continuing table right after its last rows:
/// on the page where the rows end (that piece shrunk to what it needs), or on a
/// page of their own when they don't fit there. They are mirrors (`opts.mirror`)
/// of the designed widgets, which stay where they were drawn.
fn place_linked(doc: &mut Doc, linked: &[(String, String, u32, Vec<Widget>)], results: &std::collections::HashMap<String, Json>) {
    let ch = vm::chains(doc, results);
    for (sid, host, top, followers) in linked {
        let top = *top;
        let data = results.get(sid);
        let n = data.and_then(|d| d.get("rows")).and_then(|r| r.as_array()).map(|r| r.len()).unwrap_or(0) as u32;
        // the piece where the rows end
        let mut at: Option<(usize, usize, vm::Chain)> = None;
        'find: for (pi, p) in doc.pages.iter().enumerate() {
            for (wi, w) in p.widgets.iter().enumerate() {
                if vm::continues(w) == Some(sid.as_str()) {
                    if let Some(c) = ch.get(&w.id).filter(|c| c.last) {
                        at = Some((pi, wi, *c));
                        break 'find;
                    }
                }
            }
        }
        let Some((pi, wi, c)) = at else {
            // it didn't continue after all: the widgets stay where they are
            doc.grow.remove(host);
            for f in followers {
                doc.moved.remove(&f.id);
            }
            continue;
        };
        // an automatic piece only takes the rows it needs
        if doc.pages[pi].auto {
            let piece = doc.pages[pi].widgets[wi].clone();
            for h in 1..=piece.h {
                let f = doc.page.frame(piece.x, piece.y, piece.w, h);
                if vm::widget_vm_chain(doc, &piece, f, data.cloned(), c).table.is_some_and(|t| c.offset + t.visible >= n) {
                    doc.pages[pi].widgets[wi].h = h;
                    break;
                }
            }
        }
        let piece = doc.pages[pi].widgets[wi].clone();
        let tall = followers.iter().map(|f| f.y - top + f.h).max().unwrap_or(0);
        let base = piece.y + piece.h;
        let free = base + tall <= doc.page.rows
            && !doc.pages[pi].widgets.iter().any(|o| o.id != piece.id && o.y < base + tall && o.y + o.h > base && followers.iter().any(|f| o.x < f.x + f.w && o.x + o.w > f.x));
        let (page, y0) = if free {
            (pi, base)
        } else {
            // on a page of their own, right after
            let id = doc.new_id("p");
            let (title, section, row) = (doc.pages[pi].title.clone(), doc.pages[pi].section.clone(), doc.pages[pi].row.clone());
            doc.pages.insert(pi + 1, Page { id, title, widgets: vec![], auto: true, section, plain: false, row, ..Default::default() });
            (pi + 1, 0)
        };
        for f in followers {
            let mut m = f.clone();
            m.id = format!("{}^m", f.id);
            m.y = y0 + (f.y - top);
            m.locked = true;
            if let Some(o) = m.opts.as_object_mut() {
                o.remove("follows");
                o.insert("mirror".into(), json!(f.id));
            }
            doc.pages[page].widgets.push(m);
        }
    }
}

/// A continuation piece's options: the source's, linked back to it.
fn continuation_opts(src: &Json, src_id: &str) -> Json {
    let mut opts = if src.is_object() { src.clone() } else { json!({}) };
    let o = opts.as_object_mut().unwrap();
    let t = o.entry("table").or_insert_with(|| json!({}));
    if !t.is_object() {
        *t = json!({});
    }
    let t = t.as_object_mut().unwrap();
    t.insert("continues".into(), json!(src_id));
    t.insert("paginate".into(), json!(false));
    opts
}

/// Turns a template into a fresh report.
fn to_report(doc: &mut Doc, template_dir: Option<&Path>) {
    doc.kind = DocKind::Report;
    report_catalog(doc);
    doc.meta.template_title = doc.meta.title.clone();
    doc.meta.created = chrono::Local::now().format("%Y-%m-%d").to_string();
    // tidy after changes, but no stretching into leftover space (a designed page keeps its proportions)
    doc.arrange = layout::Options { auto: true, fill: false };
    // relative data paths were relative to the template
    if let Some(dir) = template_dir {
        for d in &mut doc.datasets {
            match &mut d.source {
                model::Source::Csv { path } | model::Source::Excel { path, .. } | model::Source::Sqlite { path, .. } => {
                    *path = data::resolve_path(path, Some(dir)).display().to_string();
                }
                _ => {}
            }
        }
    }
}

/// What a report's user can add besides the template's gallery: the template's
/// own charts, tables and figures, by page — so a figure removed can come back,
/// or show twice in two ways. A figure of a repeated page follows the page's
/// row (its account), so it goes only on that page.
fn report_catalog(doc: &mut Doc) {
    let mut add = Vec::new();
    for p in doc.pages.iter().filter(|p| p.repeat_of.is_empty() && !p.auto) {
        for w in p.widgets.iter().filter(|w| w.binding.is_some() && CHART_KINDS.contains(&w.kind.as_str()) && !w.title.trim().is_empty()) {
            let id = format!("tpl-{}", w.id);
            if doc.catalog.iter().any(|c| c.id == id) {
                continue;
            }
            let mut w = w.clone();
            w.locked = false;
            add.push(model::CatalogItem { id, name: w.title.clone(), description: String::new(), category: p.title.clone(), widget: w, page: p.id.clone() });
        }
    }
    doc.catalog.extend(add);
}

/// Sizes each studio preview to its content, so tables and text show whole.
fn fit_studio(doc: &mut Doc, results: &std::collections::HashMap<String, Json>) {
    let rows = doc.page.rows;
    for pi in 0..doc.pages.len() {
        let items = vm::items(doc, &doc.pages[pi], results);
        for (w, it) in doc.pages[pi].widgets.iter_mut().zip(items) {
            if let (true, Some(need)) = (theme_studio::FITTED.contains(&w.kind.as_str()), it.need_h) {
                w.h = need.clamp(1, rows);
            }
        }
    }
}

fn read_doc(path: &Path) -> Result<Doc, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    if let Some(tf) = read_theme_text(&text) {
        return Ok(theme_studio::studio_doc(tf.theme, tf.fonts));
    }
    let doc: Doc = serde_json::from_str(&text).map_err(|e| format!("{} is not a Quire file: {e}", path.display()))?;
    if doc.format > model::FORMAT {
        return Err("This file was made by a newer version of Quire.".into());
    }
    Ok(doc)
}

fn read_theme_text(text: &str) -> Option<theme_studio::ThemeFile> {
    let tf: theme_studio::ThemeFile = serde_json::from_str(text).ok()?;
    (tf.format == theme_studio::THEME_FORMAT).then_some(tf)
}

fn theme_library(host: &dyn Host) -> Option<PathBuf> {
    let dir = host.data_dir()?.join("themes");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Fonts a theme uses that the document carries.
fn theme_fonts(doc: &Doc) -> Vec<model::EmbeddedFont> {
    let used: std::collections::HashSet<String> = std::iter::once(doc.theme.font_family.clone()).chain(doc.theme.fonts.values().filter_map(|f| f.family.clone())).collect();
    doc.fonts.iter().filter(|f| used.contains(&f.family)).cloned().collect()
}

/// Themes to choose from: built-in ones and those saved to the library.
fn list_themes(state: St<'_>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let app = lock(&state);
    gate(&app, &["design", "user"])?;
    Ok(theme_list(theme_library(app_handle).as_deref(), app.doc.as_ref()))
}

/// The built-in and library themes, each with its colours and whether it has
/// every design view the open document uses (`fits`: switching keeps every look).
fn theme_list(lib: Option<&Path>, doc: Option<&Doc>) -> Json {
    let used: std::collections::BTreeSet<(String, String)> = doc
        .map(|d| d.pages.iter().flat_map(|p| &p.widgets).filter(|w| !w.variant.is_empty()).map(|w| (w.kind.clone(), w.variant.clone())).collect())
        .unwrap_or_default();
    let item = |id: String, t: &model::Theme, builtin: bool| {
        let fits = used.iter().all(|(k, v)| t.styles.get(k).is_some_and(|l| l.iter().any(|x| x.id == *v)));
        let colors: Vec<&String> = [&t.page_bg, &t.ink, &t.accent].into_iter().chain(t.palette.iter().take(4)).collect();
        // its typefaces: headings, then text
        let heading = t.fonts.get("heading").and_then(|f| f.family.clone()).unwrap_or_else(|| t.font_family.clone());
        json!({"id": id, "name": t.name, "builtin": builtin, "views": t.styles.values().map(|l| l.len()).sum::<usize>(), "fits": fits, "colors": colors,
               "fonts": [heading, t.font_family.clone()],
               // the facts a designer compares themes by: card style and corners, the chart look, gains and losses
               "card": t.card, "card_bg": t.card_bg, "radius": t.radius, "chart_look": t.chart_look, "good": t.good, "bad": t.bad,
               "kinds": t.styles.iter().filter(|(_, l)| !l.is_empty()).count(), "restyled": t.look.get("views").is_some_and(|v| v.as_object().is_some_and(|m| !m.is_empty())),
               "current": doc.is_some_and(|d| d.theme.name == t.name)})
    };
    let mut out: Vec<Json> = theme_studio::builtins().into_iter().filter_map(|(id, _)| theme_studio::builtin(id).map(|t| item(format!("builtin:{id}"), &t, true))).collect();
    let mut files: Vec<PathBuf> = lib.and_then(|d| std::fs::read_dir(d).ok()).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "qtheme")).collect()).unwrap_or_default();
    files.sort();
    for p in files {
        if let Ok((t, _)) = load_theme(&format!("file:{}", p.display())) {
            let mut v = item(format!("file:{}", p.display()), &t, false);
            v["path"] = json!(p.display().to_string());
            v["modified"] = json!(std::fs::metadata(&p).ok().and_then(|m| m.modified().ok()).map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339()).unwrap_or_default());
            out.push(v);
        }
    }
    Json::Array(out)
}

fn load_theme(id: &str) -> Result<(model::Theme, Vec<model::EmbeddedFont>), String> {
    if let Some(b) = id.strip_prefix("builtin:") {
        return theme_studio::builtin(b).map(|t| (t, Vec::new())).ok_or_else(|| format!("No built-in theme '{b}'."));
    }
    let path = id.strip_prefix("file:").unwrap_or(id);
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not open the theme: {e}"))?;
    let tf = read_theme_text(&text).ok_or("That file is not a Quire theme.")?;
    Ok((tf.theme, tf.fonts))
}

/// A free `<name>.qtheme` path in the library (`keep` may be reused: it is the file being renamed).
fn library_path(dir: &Path, name: &str, keep: Option<&Path>) -> PathBuf {
    let base = sanitize(name);
    (1..)
        .map(|n| dir.join(if n == 1 { format!("{base}.qtheme") } else { format!("{base} {n}.qtheme") }))
        .find(|p| !p.exists() || Some(p.as_path()) == keep)
        .unwrap()
}

/// Rename, duplicate or delete a theme in the library. `open` is the file the
/// theme studio has open, which is left alone. Returns the new theme's id.
fn library_op(dir: &Path, op: &str, id: &str, name: &str, open: Option<&Path>) -> Result<Json, String> {
    let file = id.strip_prefix("file:").map(PathBuf::from);
    if let Some(f) = &file {
        if !f.starts_with(dir) {
            return Err("Only themes in your library can be changed here.".into());
        }
        if op != "duplicate" && open == Some(f.as_path()) {
            return Err("That theme is open in the theme studio. Close it first.".into());
        }
    }
    let name = name.trim();
    let write = |theme: model::Theme, fonts: Vec<model::EmbeddedFont>, path: &Path| -> Result<(), String> {
        let tf = theme_studio::ThemeFile { format: theme_studio::THEME_FORMAT.into(), theme, fonts };
        std::fs::write(path, serde_json::to_string_pretty(&tf).map_err(|e| e.to_string())?).map_err(|e| format!("Could not save the theme: {e}"))
    };
    match op {
        "delete" => {
            let f = file.ok_or("Built-in themes cannot be deleted.")?;
            std::fs::remove_file(&f).map_err(|e| format!("Could not delete the theme: {e}"))?;
            Ok(Json::Null)
        }
        "rename" => {
            let f = file.ok_or("Built-in themes cannot be renamed. Duplicate one to make your own.")?;
            if name.is_empty() {
                return Err("A theme needs a name.".into());
            }
            let (mut theme, fonts) = load_theme(id)?;
            theme.name = name.into();
            let to = library_path(dir, name, Some(&f));
            write(theme, fonts, &to)?;
            if to != f {
                let _ = std::fs::remove_file(&f);
            }
            Ok(json!(format!("file:{}", to.display())))
        }
        "duplicate" => {
            let (mut theme, fonts) = load_theme(id)?;
            theme.name = if name.is_empty() { format!("{} copy", theme.name) } else { name.into() };
            let to = library_path(dir, &theme.name, None);
            write(theme, fonts, &to)?;
            Ok(json!(format!("file:{}", to.display())))
        }
        _ => Err(format!("Unknown theme action '{op}'.")),
    }
}

/// Renames, duplicates or deletes a theme in the library.
fn theme_library_op(state: St<'_>, op: String, id: String, name: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    gate(&lock(&state), &["design"])?;
    let dir = theme_library(app_handle).ok_or("The theme library is not available.")?;
    let open = lock(&state).path.clone().filter(|p| p.extension().is_some_and(|e| e == "qtheme"));
    library_op(&dir, &op, &id, name.as_deref().unwrap_or(""), open.as_deref())
}

/// Opens the theme studio: on the open template's theme ("current"), a theme
/// from the library, or a new one. The template is set aside until the studio closes.
fn studio_open(app: &mut App, source: &str) -> Result<Json, String> {
    app.room()?;
    let from = app.doc.as_ref().filter(|d| d.kind != DocKind::Theme).map(|_| app.active_id);
    // this document's studio is open already: go back to it
    if source == "current" {
        if let Some(id) = from.and_then(|f| app.parked.iter().find(|s| s.doc.kind == DocKind::Theme && s.link == Some(f)).map(|s| s.id)) {
            app.activate(id)?;
            return Ok(app.resp());
        }
    }
    let (theme, fonts) = match source {
        "current" => {
            let d = app.doc.as_ref().ok_or("No template is open.")?;
            let mut t = d.theme.clone();
            if t.name.is_empty() {
                t.name = format!("{} theme", d.meta.title);
            }
            (t, theme_fonts(d))
        }
        "new" => (model::Theme { name: "New theme".into(), ..model::Theme::default() }, Vec::new()),
        other => load_theme(other)?,
    };
    app.load(theme_studio::studio_doc(theme, fonts), None);
    // the studio of the shown document is linked to it (its theme travels)
    if source == "current" {
        if let Some(f) = from {
            app.active_link = Some(f);
            if let Some(s) = app.parked.iter_mut().find(|s| s.id == f) {
                s.link = Some(app.active_id);
            }
        }
    }
    app.dirty = source == "new";
    app.studio_base = app.doc.as_ref().and_then(|d| serde_json::to_value(&d.theme).ok());
    Ok(app.resp())
}

/// Switches between the theme studio and its document (both stay open; the theme travels).
fn studio_swap(app: &mut App) -> Result<Json, String> {
    let target = if app.doc.as_ref().is_some_and(|d| d.kind == DocKind::Theme) {
        app.active_link
    } else {
        app.parked.iter().find(|s| s.doc.kind == DocKind::Theme && s.link == Some(app.active_id)).map(|s| s.id)
    };
    app.activate(target.ok_or("The theme studio and its document are not both open.")?)?;
    Ok(app.resp())
}

/// Closes the theme studio, applying its theme to its document or not, and shows that document.
fn studio_close(app: &mut App, apply: bool) -> Json {
    let studio = app.active_id;
    match app.active_link.filter(|l| app.parked.iter().any(|s| s.id == *l)) {
        Some(l) if apply => {
            let _ = app.activate(l);
            app.close_doc(studio);
        }
        Some(l) => {
            app.doc = None;
            app.active_link = None;
            app.unlink(studio);
            let _ = app.activate(l);
        }
        None => app.close_doc(studio),
    }
    app.resp()
}

fn apply_theme_to(app: &mut App, id: &str) -> Result<Json, String> {
    let (theme, fonts) = load_theme(id)?;
    let doc = app.doc.as_mut().ok_or("No template is open.")?;
    match doc.kind {
        DocKind::Template => {}
        // a report takes another theme when its template lets users change the look
        DocKind::Report if doc.policy.restyle => {}
        DocKind::Report => return Err("This report's template doesn't allow changing the look.".into()),
        _ => return Err("Themes are set on templates and reports.".into()),
    }
    let before = doc.clone();
    theme_studio::apply(doc, &theme, &fonts);
    looks::doc(doc);
    proposal::fit_views(doc);
    app.undo.push(before);
    app.redo.clear();
    app.dirty = true;
    app.register_fonts();
    // another theme sizes tables differently: their pieces and continuation pages follow
    app.sync_chains();
    Ok(app.resp())
}

/// Opens the theme studio: on the open template's theme ("current"), a theme
/// from the library, or a new one. The template is set aside until the studio closes.
fn theme_studio_open(state: St<'_>, source: String) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["design"])?;
    studio_open(&mut app, &source)
}

/// Leaves the studio, back to the template set aside (applying the theme to it when asked).
/// Shows another open document.
fn doc_activate(state: St<'_>, id: u64) -> Result<Json, String> {
    let mut app = lock(&state);
    app.activate(id)?;
    Ok(app.resp())
}

/// Closes an open document (the UI asks first about unsaved changes).
/// `doc_close`, recorded in the activity log.
fn doc_close(state: St<'_>, id: u64) -> Result<Json, String> {
    let title = { let a = lock(&state); a.open_title(id) };
    let sid = { let a = lock(&state); let w = a.work.borrow(); w.open.get(&id).map(|w| w.id.clone()).unwrap_or_default() };
    let r = doc_close_run(state, id);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit_in(&sid, "document", "close", &title, "Closed", Json::Null, err);
        app.end_work_of(Some(id), "closed");
    }
    r
}

fn doc_close_run(state: St<'_>, id: u64) -> Result<Json, String> {
    let mut app = lock(&state);
    app.close_doc(id);
    Ok(app.resp())
}

fn theme_studio_swap(state: St<'_>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["design"])?;
    studio_swap(&mut app)
}

fn theme_studio_close(state: St<'_>, apply: bool) -> Result<Json, String> {
    Ok(studio_close(&mut lock(&state), apply))
}

/// Puts a theme from the library on the open template.
/// `apply_theme`, recorded in the activity log.
fn apply_theme(state: St<'_>, id: String) -> Result<Json, String> {
    let tid = id.clone();
    let r = apply_theme_run(state, id);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("edit", "apply theme", "", &tid, Json::Null, err);
    }
    r
}

fn apply_theme_run(state: St<'_>, id: String) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, theme_roles(app.doc.as_ref()))?;
    apply_theme_to(&mut app, &id)
}

/// `open_path`, recorded in the activity log.
fn open_path(state: St<'_>, path: String) -> Result<Json, String> {
    let pth = path.clone();
    let r = open_path_run(state, path);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("document", "open", "", &pth, Json::Null, err);
    }
    r
}

fn open_path_run(state: St<'_>, path: String) -> Result<Json, String> {
    let p = PathBuf::from(&path);
    if !state.host().may_open(&p) {
        return Err("That file is outside Quire's folder.".into());
    }
    let mut app = lock(&state);
    // already open: show it
    if let Some(id) = app.open_at(&p) {
        app.activate(id)?;
        return Ok(app.resp());
    }
    let doc = read_doc(&p)?;
    gate_open(&app, &doc)?;
    app.room()?;
    app.load(doc, Some(p));
    Ok(app.resp())
}

/// `open_dialog`, recorded in the activity log.
fn open_dialog(state: St<'_>) -> Result<Option<Json>, String> {
    let app_handle: H = state.host();
    
    let r = open_dialog_run(app_handle, state);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        if r.as_ref().map(|x| x.is_some()).unwrap_or(true) { app.audit("document", "open", "", &app.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(), Json::Null, err); }
    }
    r
}

fn open_dialog_run(app_handle: H, state: St<'_>) -> Result<Option<Json>, String> {
    let picked = app_handle
        .dialog()
        .file()
        .add_filter("Quire files", &["rtpl", "rpt", "qtheme"])
        .add_filter("Templates", &["rtpl"])
        .add_filter("Reports", &["rpt"])
        .blocking_pick_file()?;
    let Some(fp) = picked else { return Ok(None) };
    let p = fp.into_path().map_err(|e| e.to_string())?;
    let mut app = lock(&state);
    if let Some(id) = app.open_at(&p) {
        app.activate(id)?;
        return Ok(Some(app.resp()));
    }
    let doc = read_doc(&p)?;
    gate_open(&app, &doc)?;
    app.room()?;
    app.load(doc, Some(p));
    Ok(Some(app.resp()))
}

/// Picks a template file and starts a new report from it.
/// `report_from_template`, recorded in the activity log.
fn report_from_template(state: St<'_>, current: bool) -> Result<Option<Json>, String> {
    let app_handle: H = state.host();
    
    let r = report_from_template_run(app_handle, state, current);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        if r.as_ref().map(|x| x.is_some()).unwrap_or(true) { app.audit("document", "new report", "", "Report made from the template", Json::Null, err); }
    }
    r
}

fn report_from_template_run(app_handle: H, state: St<'_>, current: bool) -> Result<Option<Json>, String> {
    gate(&lock(&state), &["user", "design"])?;
    let (doc, dir) = if current {
        let app = lock(&state);
        let d = app.doc.clone().ok_or("No document is open.")?;
        (d, app.base_dir())
    } else {
        let picked = app_handle.dialog().file().add_filter("Templates", &["rtpl"]).blocking_pick_file()?;
        let Some(fp) = picked else { return Ok(None) };
        let p = fp.into_path().map_err(|e| e.to_string())?;
        (read_doc(&p)?, p.parent().map(Path::to_path_buf))
    };
    let mut doc = doc;
    if doc.kind == DocKind::Report {
        return Err("That file is already a report.".into());
    }
    to_report(&mut doc, dir.as_deref());
    let mut app = lock(&state);
    app.room()?;
    app.load(doc, None);
    app.dirty = true;
    Ok(Some(app.resp()))
}

/// `save`, recorded in the activity log.
fn save(state: St<'_>, save_as: bool) -> Result<Option<Json>, String> {
    let app_handle: H = state.host();
    
    let r = save_run(app_handle, state, save_as);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        if r.as_ref().map(|x| x.is_some()).unwrap_or(true) { app.audit("document", "save", "", &app.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(), Json::Null, err); }
    }
    r
}

fn save_run(app_handle: H, state: St<'_>, save_as: bool) -> Result<Option<Json>, String> {
    let (doc, path) = {
        let app = lock(&state);
        (app.doc.clone().ok_or("No document is open.")?, app.path.clone())
    };
    if doc.kind == DocKind::Theme {
        let tf = theme_studio::ThemeFile { format: theme_studio::THEME_FORMAT.into(), theme: doc.theme.clone(), fonts: doc.fonts.clone() };
        let text = serde_json::to_string_pretty(&tf).map_err(|e| e.to_string())?;
        let lib = theme_library(app_handle);
        let path = match path.filter(|p| !save_as && p.extension().is_some_and(|e| e == "qtheme")) {
            Some(p) => p,
            None => {
                let mut b = app_handle.dialog().file().set_file_name(format!("{}.qtheme", sanitize(&doc.theme.name))).add_filter("Quire theme", &["qtheme"]);
                if let Some(l) = &lib {
                    b = b.set_directory(l);
                }
                let Some(fp) = b.blocking_save_file()? else { return Ok(None) };
                let mut p = fp.into_path().map_err(|e| e.to_string())?;
                p.set_extension("qtheme");
                p
            }
        };
        std::fs::write(&path, &text).map_err(|e| format!("Could not save: {e}"))?;
        // keep a copy in the library so templates can pick it
        if let Some(l) = lib.filter(|l| !path.starts_with(l)) {
            let _ = std::fs::write(l.join(format!("{}.qtheme", sanitize(&doc.theme.name))), &text);
        }
        let mut app = lock(&state);
        app.path = Some(path);
        app.dirty = false;
        let mut st = app.store();
        if app.record_usage(&mut st) {
            let _ = app.save_store(&st);
        }
        return Ok(Some(app.resp()));
    }
    let ext = if doc.kind == DocKind::Template { "rtpl" } else { "rpt" };
    // a template saved the first time goes to the templates library (Save as… picks a place)
    let lib = if doc.kind == DocKind::Template { templates_dir(&lock(&state)) } else { None };
    let path = match path.filter(|p| !save_as && p.extension().is_some_and(|e| e == ext)) {
        Some(p) => p,
        None if !save_as && lib.is_some() => template_path(lib.as_ref().unwrap(), &doc.meta.title),
        None => {
            // a report is named for its client too: every client's report has the template's title
            let client = if doc.kind == DocKind::Report { doc_client(&doc) } else { None };
            let name = format!("{}.{ext}", sanitize(&client.map(|c| format!("{} — {c}", doc.meta.title)).unwrap_or_else(|| doc.meta.title.clone())));
            let mut dlg = app_handle.dialog().file();
            if let Some(l) = &lib {
                dlg = dlg.set_directory(l);
            }
            let picked = dlg
                .set_file_name(&name)
                .add_filter(if ext == "rtpl" { "Quire template" } else { "Quire report" }, &[ext])
                .blocking_save_file()?;
            let Some(fp) = picked else { return Ok(None) };
            let mut p = fp.into_path().map_err(|e| e.to_string())?;
            if p.extension().is_none_or(|e| e != ext) {
                p.set_extension(ext);
            }
            p
        }
    };
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("Could not save: {e}"))?;
    let mut app = lock(&state);
    app.path = Some(path);
    app.dirty = false;
    Ok(Some(app.resp()))
}

fn sanitize(s: &str) -> String {
    let t: String = s.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    let t = t.trim();
    if t.is_empty() {
        "Report".into()
    } else {
        t.to_string()
    }
}

fn pick_data_file(state: St<'_>, kind: String) -> Result<Option<String>, String> {
    let app_handle: H = state.host();
    let exts: &[&str] = match kind.as_str() {
        "csv" => &["csv", "tsv", "txt"],
        "excel" => &["xlsx", "xlsm", "xls", "ods"],
        "sqlite" => &["db", "sqlite", "sqlite3"],
        "duckdb" => &["duckdb", "ddb", "db"],
        "image" => &["png", "jpg", "jpeg", "svg"],
        "font" => &["ttf", "otf", "ttc"],
        _ => &["*"],
    };
    let picked = app_handle.dialog().file().add_filter("Data", exts).blocking_pick_file()?;
    Ok(picked.and_then(|f| f.into_path().ok()).map(|p| p.display().to_string()))
}

/// A file from the person's computer, sent by the browser version (base64), kept in the app's
/// `uploads` folder: its path there, which data sources, images and fonts then use as on the
/// desktop. The same file again reuses its copy; another one of the same name gets " (2)".
fn upload_file(state: St<'_>, name: String, data: String) -> Result<String, String> {
    use base64::Engine as _;
    gate(&lock(state), &[])?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|_| "The file is unreadable.".to_string())?;
    if bytes.len() > 200_000_000 {
        return Err("Files must be smaller than 200 MB.".into());
    }
    let dir = ai_dir(state.host())?.join("uploads");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not keep the file: {e}"))?;
    // only the file's name, never a folder the browser names
    let name = Path::new(&name).file_name().map(|n| sanitize(&n.to_string_lossy())).unwrap_or_else(|| "upload".into());
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    for n in 1.. {
        let p = dir.join(if n == 1 { name.clone() } else { format!("{stem} ({n}){ext}") });
        match std::fs::read(&p) {
            Ok(old) if old == bytes => return Ok(p.display().to_string()),
            Ok(_) => continue,
            Err(_) => {
                std::fs::write(&p, &bytes).map_err(|e| format!("Could not keep the file: {e}"))?;
                return Ok(p.display().to_string());
            }
        }
    }
    unreachable!()
}

/// A file the browser version just wrote (an export, a brief, a CSV), for the page to download:
/// its name and bytes (base64). Only files in the app's `files` folder.
fn saved_file(state: St<'_>, path: String) -> Result<Json, String> {
    use base64::Engine as _;
    gate(&lock(state), &[])?;
    // only the person's own files (the browser version's); none on the desktop
    let dir = state.host().files_dir().and_then(|d| d.canonicalize().ok()).ok_or("No files to download.")?;
    let p = Path::new(&path).canonicalize().map_err(|_| "That file is gone.".to_string())?;
    if !p.starts_with(&dir) || !p.is_file() {
        return Err("Only your own saved files can be downloaded.".into());
    }
    let bytes = std::fs::read(&p).map_err(|e| format!("Could not read {}: {e}", p.display()))?;
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(json!({"name": name, "data": base64::engine::general_purpose::STANDARD.encode(bytes)}))
}

/// A connection as the page sent it, with its password back where the page saw it hidden.
fn real_conn_in(st: &auth::Store, c: model::Connection) -> model::Connection {
    if auth::split_password(&c.url).1.as_deref() != Some(auth::HIDDEN) {
        return c;
    }
    match st.connections.iter().find(|x| x.id == c.id).and_then(|x| auth::split_password(&x.url).1) {
        Some(pw) => model::Connection { url: auth::with_password(&c.url, &pw), ..c },
        None => model::Connection { url: auth::split_password(&c.url).0, ..c },
    }
}

fn real_conn(app: &App, c: model::Connection) -> model::Connection {
    if crate::secrets::enabled() { real_conn_in(&app.store(), c) } else { c }
}

/// The client a report is for (its client parameter's value), if it has one.
fn doc_client(doc: &Doc) -> Option<String> {
    doc.param_defs.iter().filter(|d| auth::is_client_param(d)).find_map(|d| doc.params.values.get(&d.name)).map(|v| model::param_values(v).join(", ")).filter(|c| !c.is_empty() && c != "All")
}

/// The person's saved reports, templates and themes in the browser version (their own folder),
/// newest first: path, file name, title, kind, client, when saved. Empty on the desktop.
fn saved_documents(state: St<'_>) -> Result<Json, String> {
    gate(&lock(state), &[])?;
    let Some(dir) = state.host().files_dir() else { return Ok(json!([])) };
    let mut out: Vec<(std::time::SystemTime, Json)> = std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).collect::<Vec<_>>()).unwrap_or_default().into_iter().filter_map(|e| {
        let p = e.path();
        let ext = p.extension()?.to_str()?.to_string();
        if !matches!(ext.as_str(), "rpt" | "rtpl" | "qtheme") {
            return None;
        }
        let at = e.metadata().ok()?.modified().ok()?;
        let d = read_doc(&p).ok()?;
        let kind = match d.kind { DocKind::Report => "report", DocKind::Theme => "theme", _ => "template" };
        let title = if d.kind == DocKind::Theme { d.theme.name.clone() } else { d.meta.title.clone() };
        Some((at, json!({"path": p.display().to_string(), "file": p.file_name()?.to_string_lossy(), "title": title, "kind": kind,
            "client": doc_client(&d), "saved": chrono::DateTime::<chrono::Local>::from(at).to_rfc3339()})))
    }).collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(Json::Array(out.into_iter().map(|(_, j)| j).collect()))
}

/// Deletes one of the person's saved documents (browser version; not one that is open).
fn delete_saved(state: St<'_>, path: String) -> Result<Json, String> {
    let app = lock(state);
    gate(&app, &[])?;
    let dir = state.host().files_dir().and_then(|d| d.canonicalize().ok()).ok_or("Nothing to delete here.")?;
    let p = Path::new(&path).canonicalize().map_err(|_| "That file is gone.".to_string())?;
    if !p.starts_with(&dir) {
        return Err("Only your own saved files can be deleted.".into());
    }
    if app.open_at(&p).is_some() {
        return Err("It is open. Close it first.".into());
    }
    std::fs::remove_file(&p).map_err(|e| format!("Could not delete it: {e}"))?;
    app.audit("document", "delete", "", &format!("Deleted {}", p.display()), Json::Null, None);
    Ok(Json::Null)
}

fn read_image(_state: St<'_>, path: String) -> Result<String, String> {
    use base64::Engine as _;
    data::check_file(Path::new(&path))?;
    let bytes = std::fs::read(&path).map_err(|e| format!("Could not read image: {e}"))?;
    if bytes.len() > 4_000_000 {
        return Err("Images must be smaller than 4 MB.".into());
    }
    let mime = match Path::new(&path).extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => "image/jpeg",
    };
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn excel_sheets(_state: St<'_>, path: String) -> Result<Vec<String>, String> {
    use calamine::Reader;
    data::check_file(Path::new(&path))?;
    let wb = calamine::open_workbook_auto(&path).map_err(|e| e.to_string())?;
    Ok(wb.sheet_names().to_vec())
}

/// Loads a dataset definition and returns its first rows and schema.
fn preview_dataset(state: St<'_>, dataset: Dataset, keep: Option<bool>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["design", "data"])?;
    let base = app.base_dir();
    let doc = preview_doc(&app);
    let App { engine, py, .. } = &mut *app;
    // a source being edited loads afresh; a saved one may come from the cache
    if !keep.unwrap_or(false) {
        engine.clear();
    }
    let t = engine.table_all(&doc, &dataset, base.as_deref(), py)?;
    let mut p = t.preview(60);
    p["snapshot_hint"] = json!(looks_like_snapshots(&t, dataset.date_field.as_deref()));
    Ok(p)
}

/// Where published reports' pages are kept (next to the accounts).
fn reports_dir(app: &App) -> Option<PathBuf> {
    app.store_path.as_ref().and_then(|p| p.parent()).map(|d| d.join("reports"))
}

/// The widget types library: the designers' own widget types, one file each, for every template.
fn widget_types_dir(app: &App) -> Option<PathBuf> {
    let d = app.store_path.as_ref().and_then(|p| p.parent()).map(|d| d.join("widget_types"))?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn read_widget_type(dir: &Path, id: &str) -> Option<model::WidgetType> {
    if id.contains('/') || id.contains("..") { return None; }
    serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{id}.json"))).ok()?).ok()
}

fn list_widget_types(dir: &Path) -> Vec<model::WidgetType> {
    let mut v: Vec<model::WidgetType> = std::fs::read_dir(dir).map(|r| r.filter_map(|e| e.ok()).filter_map(|e| {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "json") { serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok() } else { None }
    }).collect()).unwrap_or_default();
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v
}

/// The templates library: where templates are saved unless the designer picks another place
/// (the Design area lists them, with their published reports).
fn templates_dir(app: &App) -> Option<PathBuf> {
    let d = app.store_path.as_ref().and_then(|p| p.parent()).map(|d| d.join("templates"))?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// A free `<name>.rtpl` in the library.
fn template_path(dir: &Path, name: &str) -> PathBuf {
    let base = sanitize(name);
    (1..).map(|n| dir.join(if n == 1 { format!("{base}.rtpl") } else { format!("{base} {n}.rtpl") })).find(|p| !p.exists()).unwrap()
}

/// Writes the open template to its file — its own, or a new one in the library — and marks it saved.
fn save_template_here(app: &mut App) -> Result<PathBuf, String> {
    let doc = app.doc.as_ref().ok_or("No template is open.")?;
    let path = match app.path.clone().filter(|p| p.extension().is_some_and(|e| e == "rtpl")) {
        Some(p) => p,
        None => template_path(&templates_dir(app).ok_or("The templates library is not available.")?, &doc.meta.title),
    };
    std::fs::write(&path, serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?).map_err(|e| format!("Could not save: {e}"))?;
    app.path = Some(path.clone());
    app.dirty = false;
    Ok(path)
}

/// Whether a template's pages differ from what was published (its details aside).
fn changed_since_published(tpl: &Doc, published: &Doc) -> bool {
    let strip = |d: &Doc| { let mut v = serde_json::to_value(d).unwrap_or(Json::Null); if let Some(o) = v.as_object_mut() { o.remove("meta"); } v };
    strip(tpl) != strip(published)
}

/// The pages of a published report: its snapshot, or the built-in template it started as.
fn published_doc(app: &App, id: &str) -> Result<Doc, String> {
    let mut d = published_pages(app, id)?;
    auth::apply_report_sources(&app.store(), id, &mut d);
    Ok(d)
}

/// A published report's pages as published (without the Data area's changes).
fn published_pages(app: &App, id: &str) -> Result<Doc, String> {
    let st = app.store();
    let p = st.reports.iter().find(|r| r.id == id).ok_or("That report is no longer published.")?;
    let mut d = if let Some(f) = reports_dir(app).map(|d| d.join(format!("{id}.rtpl"))).filter(|f| f.exists()) {
        read_doc(&f)?
    } else {
        match p.builtin.as_str() {
            "uhnwi" => uhnwi_sample::template(),
            _ => return Err("This report's pages are missing — publish the template again.".into()),
        }
    };
    let _ = &mut d;
    Ok(d)
}

/// Admin → API: the accounts (no key hashes), each published report with the parameters the API
/// may set and their defaults, and how long files are kept.
fn api_admin_view(app: &App, st: &auth::Store) -> Json {
    let products: Vec<Json> = auth::options(st).map(|o| o["sources"].as_array().cloned().unwrap_or_default()).unwrap_or_default().iter().map(|s| s["id"].clone()).collect();
    let reports: Vec<Json> = st.reports.iter().map(|p| {
        let rules = st.api.reports.get(&p.id).cloned().unwrap_or_default();
        let params: Vec<Json> = report_doc(app, &p.id).map(|d| d.param_defs).unwrap_or_default().iter().filter(|d| !auth::is_client_param(d)).map(|d| {
            let r = rules.params.get(&d.name).cloned().unwrap_or_default();
            let product = d.source.as_ref().is_some_and(|s| s.field == "src_cd") || d.field == "src_cd";
            json!({"name": d.name, "label": d.label, "kind": d.kind, "multi": d.multi, "default": d.default,
                "choices": if product { json!(products) } else { json!(d.options) },
                "accept": r.accept, "api_default": r.default})
        }).collect();
        json!({"id": p.id, "title": p.title, "published": p.published, "off": rules.off, "params": params})
    }).collect();
    json!({
        "clients": st.api.clients.iter().map(|c| c.public()).collect::<Vec<_>>(),
        "targets": st.api.targets.iter().map(|t| t.public()).collect::<Vec<_>>(),
        "reports": reports,
        "retention_days": st.api.retention_days,
    })
}

/// A new report from a published template, for someone in its audience.
fn report_from_published(app: &App, id: &str) -> Result<Doc, String> {
    gate(app, &["user", "design"])?;
    let st = app.store();
    let p = st.reports.iter().find(|r| r.id == id).ok_or("That report is no longer published.")?;
    let s = app.session.as_ref().ok_or("Sign in first.")?;
    if !st.may_use(p, s) {
        return Err(format!("“{}” isn't one of your reports.", p.title));
    }
    report_doc(app, id)
}

/// A new report from a published template (whoever asks has been let through).
fn report_doc(app: &App, id: &str) -> Result<Doc, String> {
    let mut d = published_doc(app, id)?;
    to_report(&mut d, None);
    // a custom range is this year so far: 1 January to today
    if d.params.grain == "custom" {
        let today = chrono::Local::now().date_naive();
        d.params.start = chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&today), 1, 1);
        d.params.date = today;
    }
    Ok(d)
}

/// The document a source is previewed in: the open one (or an empty one),
/// with the Data area's connections added so shared sources load too.
fn preview_doc(app: &App) -> Doc {
    let mut doc = app.doc.clone().unwrap_or_else(|| {
        // a report chosen in the Data area: its sources, with the Data area's client and period
        let mut d = app.data_report.as_deref().and_then(|id| published_doc(app, id).ok()).unwrap_or_default();
        let defs = d.param_defs.clone();
        d.params = Params { values: d.params.values.clone(), ..app.data_params.clone() };
        for (k, v) in &app.data_params.values {
            d.params.values.insert(k.clone(), v.clone());
        }
        d.param_defs = defs;
        d
    });
    doc.access_src = app.session.as_ref().map(|s| s.products()).unwrap_or_default();
    for c in app.store().connections {
        if !doc.connections.iter().any(|x| x.id == c.id) {
            doc.connections.push(c);
        }
    }
    doc
}


// ------------------------------------------------------------------ AI assistants

fn ai_dir(host: &dyn Host) -> Result<PathBuf, String> {
    host.data_dir().ok_or_else(|| "The app has no folder for its files.".to_string())
}

/// What the settings page shows: providers, which keys are stored (never the keys), the bank's policy.
fn ai_status_of(app: &App, settings: &Settings) -> Result<Json, String> {
    let s = app.session.as_ref().ok_or("Sign in first.")?;
    let policy = app.store().ai;
    let providers: Vec<Json> = ai::PROVIDERS
        .iter()
        .map(|p| json!({"id": p.id, "name": p.name, "tools": p.tools, "vision": p.vision, "keys_at": p.keys_at, "endpoint": p.endpoint, "model": p.model,
            "personal": ai::key_has(p.id, &s.login), "bank": ai::key_has(p.id, ai::org()), "allowed": policy.allows(p.id)}))
        .collect();
    // what AI may send for this person, and why (their own setting, a group's or the bank's)
    let (level, from, group) = auth::ai_level_for(&app.store(), &s.login);
    Ok(json!({"providers": providers, "policy": policy, "admin": s.has("admin"), "settings": settings.ai, "my_level": {"level": level, "from": from, "group": group}}))
}

/// The provider an assistant uses: the chosen one if it has a key and is allowed, else the first that does.
fn ai_pick(app: &App, settings: &Settings, prefer: Option<&str>) -> Result<(&'static ai::Provider, String, &'static str), String> {
    let s = app.session.as_ref().ok_or("Sign in first.")?;
    let policy = app.store().ai;
    if policy.off {
        return Err("AI is switched off for this bank (Settings → AI).".into());
    }
    let first = prefer.filter(|p| !p.is_empty()).unwrap_or(settings.ai.default.as_str());
    let order = ai::PROVIDERS.iter().filter(|p| p.id == first).chain(ai::PROVIDERS.iter().filter(|p| p.id != first));
    for p in order {
        if !policy.allows(p.id) {
            continue;
        }
        if let Some((k, src)) = ai::key_for(p.id, &s.login) {
            return Ok((p, k, src));
        }
    }
    Err("No AI provider is set up yet. Add an API key in Settings → AI.".into())
}

/// The SQL assistant: write a query from a description, fix a failing one, or explain one.
/// The model sees table structure and parameter names only; the app runs what it writes
/// (rows never go back) and hands the database's error back for up to three tries.
#[allow(clippy::too_many_arguments)]
fn ai_sql_core(who: &str, settings: &Settings, picked: (&'static ai::Provider, String, &'static str), dir: &Path, conn_kind: &str, tables: &Json, params: &Json,
    lang: &str, kind: &str, prompt: &str, sql: &str, error: &str, history: &[ai::Msg], mut run: impl FnMut(&str) -> Result<Json, String>) -> Result<Json, String> {
    let (p, key, src) = picked;
    let set = settings.ai.providers.get(p.id).cloned().unwrap_or_default();
    let tmp = ai::tmp_dir(dir);
    let client = ai::Client::new(p, &set, key, &tmp);
    let ps: Vec<(String, String)> = params.as_array().into_iter().flatten().filter_map(|x| Some((x["name"].as_str()?.to_string(), x["about"].as_str().unwrap_or("").to_string()))).collect();
    let system = ai::sql_system(ai::dialect(conn_kind), &ai::schema_text(tables, 80), &ps, lang);
    let first = match kind {
        "explain" => format!("Explain this query:\n{sql}"),
        "source" => format!("Turn this query into a report source.\n\nQUERY:\n{sql}{}", if prompt.trim().is_empty() { String::new() } else { format!("\n\nAlso: {prompt}") }),
        "fix" => format!("This query fails.\n\nQUERY:\n{sql}\n\nDATABASE ERROR:\n{error}\n\nFix it.{}", if prompt.trim().is_empty() { String::new() } else { format!(" Also: {prompt}") }),
        _ => format!("TASK: {prompt}{}", if sql.trim().is_empty() { String::new() } else { format!("\n\nCURRENT QUERY (change it as the task needs):\n{sql}") }),
    };
    // the conversation so far (prompts, and the SQL and explanations the model wrote: structure only)
    let mut msgs = ai::tidy_history(history);
    let say = if lang == "ru" { "Write the explanation in Russian." } else { "Write the explanation in English." };
    msgs.push(ai::Msg::user(format!("{first}\n\n({say})")));
    let mut sent = system.len();
    let tries = if kind == "explain" { 1 } else { 3 };
    let started = std::time::Instant::now();
    let mut result: Result<Json, String> = Err("No answer.".into());
    for attempt in 1..=tries {
        sent += msgs.iter().map(|m| m.content.len()).sum::<usize>();
        let reply = match client.chat(&system, &msgs, 2000) {
            Ok(r) => r,
            Err(e) => { result = Err(e); break; }
        };
        let Some(j) = ai::parse_answer(&reply) else {
            msgs.push(ai::Msg::assistant(reply));
            msgs.push(ai::Msg::user("Answer with the JSON object only."));
            continue;
        };
        let explanation = j["explanation"].as_str().unwrap_or("").to_string();
        if kind == "explain" {
            result = Ok(json!({"ok": true, "explanation": explanation, "attempts": attempt}));
            break;
        }
        // a question about the database: answered in words, nothing to run
        if kind == "write" && j["sql"].as_str().map_or(true, |q| q.trim().is_empty()) && !explanation.is_empty() {
            result = Ok(json!({"ok": true, "explanation": explanation, "attempts": attempt}));
            break;
        }
        // turning a query into a source: what was replaced and what was kept
        let extra = if kind == "source" {
            json!({"name": j["name"], "date_column": j["date_column"], "changes": j["changes"], "left": j["left"]})
        } else {
            json!({})
        };
        let with = |mut r: Json| { if let (Some(o), Some(e)) = (r.as_object_mut(), extra.as_object()) { for (k, v) in e { if !v.is_null() { o.insert(k.clone(), v.clone()); } } } r };
        let checked = ai::read_only(j["sql"].as_str().unwrap_or(""));
        let q = match checked {
            Ok(q) => q,
            Err(e) => {
                result = Ok(with(json!({"ok": false, "sql": j["sql"], "explanation": explanation, "error": e, "attempts": attempt})));
                msgs.push(ai::Msg::assistant(reply));
                msgs.push(ai::Msg::user(format!("That isn't allowed: {e} Return a corrected JSON object.")));
                continue;
            }
        };
        match run(&q) {
            Ok(r) => {
                let cols: Vec<Json> = r["columns"].as_array().into_iter().flatten().map(|c| c["name"].clone()).collect();
                result = Ok(with(json!({"ok": true, "sql": q, "explanation": explanation, "attempts": attempt, "columns": cols, "rows": r["total"].clone()})));
                break;
            }
            Err(e) => {
                result = Ok(with(json!({"ok": false, "sql": q, "explanation": explanation, "error": e, "attempts": attempt})));
                msgs.push(ai::Msg::assistant(reply));
                msgs.push(ai::Msg::user(format!("Running it failed with this database error:\n{e}\nReturn a corrected JSON object.")));
            }
        }
    }
    let ok = result.as_ref().map(|r| r["ok"].as_bool().unwrap_or(false)).unwrap_or(false);
    ai::log_with(dir, &client, prompt, json!({"at": chrono::Local::now().to_rfc3339(), "user": who, "provider": p.id, "model": client.model, "key": src, "feature": format!("sql.{kind}"),
        "policy": "structure", "sent_chars": sent, "ok": ok, "ms": started.elapsed().as_millis() as u64, "error": result.as_ref().err()}));
    let ms = started.elapsed().as_millis() as u64;
    result.map(|mut r| { r["provider"] = json!(p.name); r["provider_id"] = json!(p.id); r["model"] = json!(client.model); r["ms"] = json!(ms); r })
}

/// Applies the design assistant's answer to the open template: the plan is checked against it,
/// applied as one undo step, and the new widgets' data and the page's fit are checked. Problems
/// come back for the model; unless it is the `last` try, the change is taken back first.
fn design_apply(app: &mut App, page: Option<&str>, after: Option<&str>, ans: &Json, last: bool) -> Result<Json, Vec<String>> {
    let doc = app.doc.as_ref().ok_or_else(|| vec!["No template is open.".to_string()])?;
    let target = page.and_then(|id| doc.pages.iter().find(|p| p.id == id));
    let plan = ai_design::plan(doc, target, ans)?;
    let before_ids: Vec<String> = doc.pages.iter().map(|p| p.id.clone()).collect();
    let undo_len = app.undo.len();
    let saved = app.doc.clone();
    let op = Edit::AiPage { page: page.map(String::from), after: after.map(String::from), title: plan.title.clone(), widgets: plan.widgets };
    let st = run_edit(app, op).map_err(|e| vec![e])?;
    let pid = page.map(String::from).or_else(|| app.doc.as_ref()?.pages.iter().find(|p| !before_ids.contains(&p.id)).map(|p| p.id.clone())).unwrap_or_default();
    // what went wrong on the page: data the new or changed widgets can't show, and what doesn't fit
    let mut issues = Vec::new();
    if let Some(pv) = st["pages"].as_array().and_then(|a| a.iter().find(|p| p["id"] == pid.as_str())) {
        for w in pv["widgets"].as_array().into_iter().flatten() {
            if let Some(e) = w["data"]["error"].as_str() {
                let title = w["title"].as_str().filter(|t| !t.is_empty()).or(w["kind"].as_str()).unwrap_or("");
                issues.push(format!("“{title}” can't show its data: {e}"));
            }
        }
        for i in pv["issues"].as_array().into_iter().flatten().filter(|i| i["severity"] != "info") {
            // with the fit advisor's own fix, so the model knows what would solve it
            let fix = i["suggestions"][0]["label"].as_str().map(|l| format!(" (the app suggests: {l})")).unwrap_or_default();
            issues.push(format!("{}{fix}", i["message"].as_str().unwrap_or("")));
        }
    }
    if !issues.is_empty() && !last {
        app.undo.truncate(undo_len);
        app.doc = saved;
        return Err(issues);
    }
    Ok(json!({"title": plan.title, "page": pid, "added": plan.added, "changed": plan.changed, "removed": plan.removed, "issues": issues, "st": st}))
}

/// The design assistant: asks for a page plan and has the app try it, up to three times.
#[allow(clippy::too_many_arguments)]
fn ai_design_core(who: &str, settings: &Settings, picked: (&'static ai::Provider, String, &'static str), dir: &Path, ctx: &str, lang: &str,
    kind: &str, prompt: &str, history: &[ai::Msg], mut try_apply: impl FnMut(&Json, bool) -> Result<Json, Vec<String>>) -> Result<Json, String> {
    let (p, key, src) = picked;
    let set = settings.ai.providers.get(p.id).cloned().unwrap_or_default();
    let tmp = ai::tmp_dir(dir);
    let client = ai::Client::new(p, &set, key, &tmp);
    let system = ai_design::system(ctx, lang);
    let task = match kind {
        "change" => format!("CHANGE THE CURRENT PAGE: {prompt}"),
        _ => format!("BUILD A NEW PAGE: {prompt}"),
    };
    let mut msgs = ai::tidy_history(history);
    let say = if lang == "ru" { "Write the explanation in Russian." } else { "Write the explanation in English." };
    msgs.push(ai::Msg::user(format!("{task}\n\n({say})")));
    let mut sent = system.len();
    let tries = 3;
    let started = std::time::Instant::now();
    let mut result: Result<Json, String> = Err("No answer.".into());
    for attempt in 1..=tries {
        sent += msgs.iter().map(|m| m.content.len()).sum::<usize>();
        let reply = match client.chat(&system, &msgs, 4000) {
            Ok(r) => r,
            Err(e) => { result = Err(e); break; }
        };
        let Some(j) = ai::parse_answer(&reply) else {
            msgs.push(ai::Msg::assistant(reply));
            msgs.push(ai::Msg::user("Answer with the JSON object only."));
            continue;
        };
        match try_apply(&j, attempt == tries) {
            Ok(mut r) => {
                r["ok"] = json!(r["issues"].as_array().is_none_or(|a| a.is_empty()));
                r["explanation"] = j["explanation"].clone();
                r["attempts"] = json!(attempt);
                r["plan"] = j["widgets"].clone();
                result = Ok(r);
                break;
            }
            Err(probs) => {
                result = Err(probs.join(" "));
                msgs.push(ai::Msg::assistant(reply));
                msgs.push(ai::Msg::user(format!("That plan doesn't work yet:\n- {}\nReturn a corrected JSON object with the whole page.", probs.join("\n- "))));
            }
        }
    }
    let ok = result.as_ref().map(|r| r["ok"].as_bool().unwrap_or(false)).unwrap_or(false);
    ai::log_with(dir, &client, prompt, json!({"at": chrono::Local::now().to_rfc3339(), "user": who, "provider": p.id, "model": client.model, "key": src, "feature": format!("design.{kind}"),
        "policy": "structure", "sent_chars": sent, "ok": ok, "ms": started.elapsed().as_millis() as u64, "error": result.as_ref().err()}));
    let ms = started.elapsed().as_millis() as u64;
    result.map(|mut r| { r["provider"] = json!(p.name); r["provider_id"] = json!(p.id); r["model"] = json!(client.model); r["ms"] = json!(ms); r })
}

/// The problems the fit advisor sees on every page (warnings and errors), as "page: message",
/// with the design view of the widget involved.
fn fit_problems(st: &Json) -> Vec<String> {
    let view = |id: &str| -> String {
        st["doc"]["pages"].as_array().into_iter().flatten().flat_map(|p| p["widgets"].as_array().into_iter().flatten())
            .find(|w| w["id"] == id).map(|w| format!(" [view {}/{}]", w["kind"].as_str().unwrap_or(""), w["variant"].as_str().unwrap_or(""))).unwrap_or_default()
    };
    st["pages"].as_array().into_iter().flatten().flat_map(|p| {
        let title = p["title"].as_str().unwrap_or("").to_string();
        let view = &view;
        p["issues"].as_array().into_iter().flatten().filter(|i| i["severity"] != "info").map(move |i| {
            let fix = i["suggestions"][0]["label"].as_str().map(|l| format!(" (the app suggests: {l})")).unwrap_or_default();
            format!("page “{title}”: {}{}{fix}", i["message"].as_str().unwrap_or(""), i["widget"].as_str().map(view).unwrap_or_default())
        })
    }).collect()
}

/// Which design views the open document's widgets use ("kind/id" → count).
fn views_used(doc: &Doc) -> std::collections::HashMap<String, usize> {
    let mut m = std::collections::HashMap::new();
    for w in doc.pages.iter().flat_map(|p| p.widgets.iter()) {
        let id = if w.variant.is_empty() { doc.theme.styles.get(&w.kind).and_then(|l| l.first()).map(|v| v.id.clone()).unwrap_or_default() } else { w.variant.clone() };
        *m.entry(format!("{}/{id}", w.kind)).or_insert(0) += 1;
    }
    m
}

/// Restyles the open template: the whole theme is built from the look the way the built-in
/// themes are, the design views' changes go on top (with the earlier restyles' kept in the
/// theme), and the page bands change. The template keeps its language and currency. One undo
/// step. Unless it is the `last` try, a restyle that makes pages break (cut-off text, overflow)
/// is taken back and its problems answered.
fn restyle_apply(app: &mut App, look: &uhnwi_theme::Look, views: &serde_json::Map<String, Json>, page: &Json, last: bool) -> Result<Json, Vec<String>> {
    let before_st = app.resp();
    let before: std::collections::HashSet<String> = fit_problems(&before_st).into_iter().collect();
    let doc = app.doc.as_mut().ok_or_else(|| vec!["No template is open.".to_string()])?;
    if doc.kind != DocKind::Template {
        return Err(vec!["Restyling works on templates.".into()]);
    }
    let mut theme = uhnwi_theme::theme_from(look);
    theme.locale = doc.theme.locale.clone();
    theme.currency = doc.theme.currency.clone();
    // the views' changes: the earlier restyles', then this one's
    let mut all = doc.theme.look.get("views").cloned().filter(|v| v.is_object()).unwrap_or_else(|| json!({}));
    model::merge_json(&mut all, &Json::Object(views.clone()));
    for (key, patch) in all.as_object().into_iter().flatten() {
        let Some((kind, id)) = key.split_once('/') else { continue };
        if let Some(v) = theme.styles.get_mut(kind).and_then(|l| l.iter_mut().find(|v| v.id == id)) {
            if patch["opts"].is_object() { model::merge_json(&mut v.opts, &patch["opts"]); }
            if patch["style"].is_object() { model::merge_json(&mut v.style, &patch["style"]); }
        }
    }
    theme.look = ai_restyle::look_json(look);
    theme.look["views"] = all;
    // the typefaces the views were given belong to the text roles (Typography changes them all)
    theme_studio::families_to_roles(&mut theme);
    // the section tabs: as they were, with this restyle's changes
    theme.tabs = doc.theme.tabs.clone();
    if page["tab_style"].is_object() {
        if !theme.tabs.is_object() { theme.tabs = json!({}); }
        model::merge_json(&mut theme.tabs, &page["tab_style"]);
    }
    // the header and footer belong to the theme too (another theme takes them back): the earlier
    // restyles' bands, then this one's
    let mut bands = doc.theme.look.get("page").cloned().filter(|v| v.is_object()).unwrap_or_else(|| json!({}));
    for band in ["header", "footer"] {
        if page[band].is_object() { model::merge_json(&mut bands, &json!({ band: page[band].clone() })); }
    }
    if bands.as_object().is_some_and(|m| !m.is_empty()) { theme.look["page"] = bands; }
    // the views' and bands' colours that repeat a named one follow it by name
    theme.link_colours();
    let saved = doc.clone();
    theme_studio::apply(doc, &theme, &[]);
    looks::doc(doc);
    app.undo.push(saved.clone());
    if app.undo.len() > UNDO_LIMIT {
        app.undo.remove(0);
    }
    app.redo.clear();
    app.dirty = true;
    app.register_fonts();
    app.sync_chains();
    let st = app.resp();
    let new: Vec<String> = fit_problems(&st).into_iter().filter(|p| !before.contains(p)).collect();
    if !new.is_empty() && !last {
        app.undo.pop();
        app.doc = Some(saved);
        app.sync_chains();
        return Err(new.into_iter().take(12).map(|p| format!("After restyling, {p}. Make text smaller, padding or row heights tighter in the views involved, or leave them as they were.")).collect());
    }
    Ok(json!({"st": st, "issues": new}))
}

/// What a reference PDF itself says (its typefaces by name and size, its exact colours by area), after the
/// person's words: facts the pictures alone can't give.
fn with_pdf_facts(prompt: &str, facts: Option<&str>) -> String {
    match facts.map(str::trim).filter(|f| !f.is_empty()) {
        Some(f) => format!("{prompt}\n\nFROM THE PDF FILE ITSELF (read by the app, exact — use them over guesses from the pictures; pick the installed typeface closest in build and weight to the file's own):\n{f}"),
        None => prompt.to_string(),
    }
}

/// The restyle assistant: asks a vision model for a look, design-view changes and page bands
/// after the `images`, and has the app check and apply them, up to three tries.
#[allow(clippy::too_many_arguments)]
fn ai_restyle_core(who: &str, settings: &Settings, picked: (&'static ai::Provider, String, &'static str), dir: &Path, base: &uhnwi_theme::Look,
    styles: &std::collections::BTreeMap<String, Vec<model::StyleVariant>>, used: &std::collections::HashMap<String, usize>, locale: &str,
    lang: &str, prompt: &str, images: &[ai::Image], history: &[ai::Msg],
    mut apply: impl FnMut(&uhnwi_theme::Look, &serde_json::Map<String, Json>, &Json, bool) -> Result<Json, Vec<String>>) -> Result<Json, String> {
    let (p, key, src) = picked;
    let set = settings.ai.providers.get(p.id).cloned().unwrap_or_default();
    let tmp = ai::tmp_dir(dir);
    let client = ai::Client::new(p, &set, key, &tmp);
    let fonts = ai_restyle::font_choices();
    let font_set: std::collections::HashSet<String> = fonts.iter().cloned().collect();
    let system = ai_restyle::system(&ai_restyle::look_json(base), &ai_restyle::views_context(styles, used), &ai_restyle::font_menu(&fonts), locale, lang);
    let mut msgs = ai::tidy_history(history);
    let say = if lang == "ru" { "Write the explanation and the changes in Russian." } else { "Write the explanation and the changes in English." };
    let n = images.len();
    let task = format!("Restyle the template after {}.{}", if n == 1 { "this picture".to_string() } else { format!("these {n} pictures (one visual language)") },
        if prompt.trim().is_empty() { String::new() } else { format!(" {prompt}") });
    msgs.push(ai::Msg::user(format!("{task}\n\n({say})")));
    let at = msgs.len() - 1;
    let mut sent = system.len() + images.iter().map(|i| i.data.len()).sum::<usize>();
    let started = std::time::Instant::now();
    let mut result: Result<Json, String> = Err("No answer.".into());
    let mut sent_back: Vec<Vec<String>> = Vec::new();
    let tries = 3;
    for attempt in 1..=tries {
        sent += msgs.iter().map(|m| m.content.len()).sum::<usize>();
        let reply = match client.chat_with(&system, &msgs, Some((images, at)), 16000) {
            Ok(r) => r,
            Err(e) => { result = Err(e); break; }
        };
        let Some(j) = ai::parse_answer(&reply) else {
            msgs.push(ai::Msg::assistant(reply));
            msgs.push(ai::Msg::user("Answer with the JSON object only."));
            continue;
        };
        let mut probs = Vec::new();
        let look = ai_restyle::look_from(base, &j, &font_set).map_err(|e| probs.extend(e)).ok();
        let views = ai_restyle::check_views(styles, &j["views"], &font_set).map_err(|e| probs.extend(e)).ok();
        let page = ai_restyle::check_page(&j["page"]).map_err(|e| probs.extend(e)).ok();
        if let (Some(look), Some(views), Some(page), true) = (look, views.as_ref(), page.as_ref(), probs.is_empty()) {
            match apply(&look, views, page, attempt == tries) {
                Ok(r) => {
                    result = Ok(json!({"ok": r["issues"].as_array().is_none_or(|a| a.is_empty()), "explanation": j["explanation"], "name": look.name, "look": ai_restyle::look_json(&look),
                        "views": views.len(), "changes": j["changes"], "issues": r["issues"], "attempts": attempt, "st": r["st"]}));
                    break;
                }
                Err(e) => probs = e,
            }
        }
        result = Err(probs.join(" "));
        sent_back.push(probs.clone());
        msgs.push(ai::Msg::assistant(reply));
        msgs.push(ai::Msg::user(format!("That restyle doesn't work yet:\n- {}\nReturn the whole corrected JSON object.", probs.join("\n- "))));
    }
    let ok = result.is_ok();
    // what went back to the model on each retry (names of settings and views, no data)
    ai::log_with(dir, &client, prompt, json!({"at": chrono::Local::now().to_rfc3339(), "user": who, "provider": p.id, "model": client.model, "key": src, "feature": "design.restyle",
        "policy": "structure", "sent_chars": sent, "ok": ok, "ms": started.elapsed().as_millis() as u64, "error": result.as_ref().err(), "sent_back": sent_back}));
    let ms = started.elapsed().as_millis() as u64;
    result.map(|mut r| { r["provider"] = json!(p.name); r["provider_id"] = json!(p.id); r["model"] = json!(client.model); r["ms"] = json!(ms); r })
}

/// Pictures from a file for the restyle: a GIF's distinct frames, or the picture itself.
fn ai_pictures(_state: St<'_>, data: String) -> Result<Json, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|_| "The file is unreadable.".to_string())?;
    (move || ai_restyle::pictures(&bytes, 8).map(Json::from))()
}

/// Restyles the open template after pictures (see `ai_restyle_core`).
#[allow(clippy::too_many_arguments)]
fn ai_restyle(state: St<'_>, images: Vec<ai::Image>, prompt: String, facts: Option<String>, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let prompt = with_pdf_facts(&prompt, facts.as_deref());
    let settings = read_settings(settings_path(app_handle).as_deref());
    let dir = ai_dir(app_handle)?;
    let (who, picked, base, styles, used, locale) = {
        let app = lock(&state);
        gate(&app, &["design"])?;
        let doc = app.doc.as_ref().ok_or("Open a template first.")?;
        if doc.kind != DocKind::Template {
            return Err("Restyling works on templates.".into());
        }
        let picked = ai_pick(&app, &settings, provider.as_deref())?;
        let who = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        (who, picked, ai_restyle::base_look(&doc.theme), doc.theme.styles.clone(), views_used(doc), doc.theme.locale.clone())
    };
    // the request belongs to the work session in front (the log ties them)
    let sid = lock(&state).work_id(true);
    (move || {
        let lang = if lang.as_deref().unwrap_or(&settings.ui_lang) == "ru" { "ru" } else { "en" };
        let apply = |look: &uhnwi_theme::Look, views: &serde_json::Map<String, Json>, page: &Json, last: bool| -> Result<Json, Vec<String>> {
            let mut app = lock(state);
            restyle_apply(&mut app, look, views, page, last)
        };
        ai::in_session_stamped(&sid, || ai_restyle_core(&who, &settings, picked, &dir, &base, &styles, &used, &locale, lang, &prompt, &images, &history.unwrap_or_default(), apply))
    })()
}

/// What the report assistant reads of the open report: its figures page by page (or one page),
/// with the client and codes masked. Refused unless the bank lets aggregates leave.
fn report_facts(app: &mut App, page: Option<&str>) -> Result<(String, ai_report::Masker, String), String> {
    // the person's own level, else their groups', else the bank's
    let login = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
    let (level, from, _) = auth::ai_level_for(&app.store(), &login);
    if !matches!(level.as_str(), "aggregates" | "full") {
        let admin = app.session.as_ref().is_some_and(|s| s.has("admin"));
        return Err(match (admin, from) {
            (true, "bank") => "AI may not use client figures yet. You can allow it for the bank.".into(),
            (true, _) => "AI may not use client figures for your account (set in Admin → Users or Groups).".into(),
            (false, "bank") => "AI may not use client figures yet. Your bank’s administrator can allow it.".into(),
            _ => "AI may not use client figures for your account. Your bank’s administrator can change this.".into(),
        });
    }
    let st = app.resp();
    let doc = app.doc.as_ref().ok_or("Open a report first.")?;
    if doc.kind != DocKind::Report {
        return Err("The report assistant works on reports.".into());
    }
    // the client, as chosen in the toolbar (and its label), is masked wherever it appears
    let mut exact: Vec<String> = Vec::new();
    for d in doc.param_defs.iter().filter(|d| auth::is_client_param(d)) {
        if let Some(v) = doc.params.values.get(&d.name) {
            exact.extend(model::param_values(v).into_iter().filter(|x| *x != "All").map(String::from));
            for x in model::param_values(v) {
                if let Some(l) = st["param_labels"][&d.name][&x].as_str() {
                    exact.push(l.to_string());
                }
            }
        }
    }
    let mut m = ai_report::Masker::new(&exact);
    let mut facts = ai_report::facts(doc, &st, page, &mut m);
    // the client's earlier proposals from the register (for the whole report, not a single page's note)
    if page.is_none() {
        if let Some(client) = exact.first() {
            let earlier = proposals::list(app.audit_dir().as_deref(), None, Some(client));
            if !earlier.is_empty() {
                facts.push_str(&format!("\nTHE BANKER'S EARLIER PROPOSALS FOR THIS CLIENT (the register; newest first)\n{}", m.mask(&proposals::for_ai(&earlier))));
            }
        }
    }
    if facts.trim().is_empty() {
        return Err("This report shows no figures to work from.".into());
    }
    Ok((facts, m, doc.theme.locale.clone()))
}

/// The report assistant: a brief and a pitch, a check before sending, or a note for a page.
#[allow(clippy::too_many_arguments)]
fn ai_report_core(who: &str, settings: &Settings, picked: (&'static ai::Provider, String, &'static str), dir: &Path, facts: &str, masker: &ai_report::Masker,
    locale: &str, lang: &str, kind: &str, prompt: &str, page_title: &str, history: &[ai::Msg]) -> Result<Json, String> {
    let (p, key, src) = picked;
    let set = settings.ai.providers.get(p.id).cloned().unwrap_or_default();
    let tmp = ai::tmp_dir(dir);
    let client = ai::Client::new(p, &set, key, &tmp);
    // reading a long report and summarising it needs little deliberation; a lower effort keeps
    // Claude's thinking short enough to leave room for the answer
    let mut client = client;
    client.effort = Some(if kind == "note" || kind == "proposal" { "medium" } else { "low" });
    let system = ai_report::system(kind, locale, lang);
    let mut msgs = ai::tidy_history(history);
    let task = match kind {
        "check" => "Check this report before it is sent.".to_string(),
        "note" => format!("Draft the note for page «{page_title}»."),
        "proposal" => "Draft a proposal for this client from the report.".to_string(),
        _ => "Prepare me for presenting this report to the client.".to_string(),
    };
    // what the banker typed may name the client, accounts or instruments: masked like the report
    let mut masker = masker.clone();
    let extra = if prompt.trim().is_empty() { String::new() } else { format!("\n\nAlso: {}", masker.mask(prompt)) };
    msgs.push(ai::Msg::user(format!("THE REPORT\n{facts}\n{task}{extra}")));
    let sent = system.len() + msgs.iter().map(|m| m.content.len()).sum::<usize>();
    let started = std::time::Instant::now();
    let mut result: Result<Json, String> = Err("No answer.".into());
    for _ in 0..2 {
        match client.chat(&system, &msgs, 8000) {
            Ok(reply) => match ai::parse_answer(&reply) {
                Some(mut j) => {
                    masker.unmask_json(&mut j);
                    result = Ok(j);
                    break;
                }
                None => {
                    msgs.push(ai::Msg::assistant(reply));
                    msgs.push(ai::Msg::user("Answer with the JSON object only."));
                }
            },
            Err(e) => { result = Err(e); break; }
        }
    }
    ai::log_with(dir, &client, prompt, json!({"at": chrono::Local::now().to_rfc3339(), "user": who, "provider": p.id, "model": client.model, "key": src, "feature": format!("report.{kind}"),
        "policy": "aggregates", "sent_chars": sent, "ok": result.is_ok(), "ms": started.elapsed().as_millis() as u64, "error": result.as_ref().err()}));
    let ms = started.elapsed().as_millis() as u64;
    result.map(|mut r| { r["provider"] = json!(p.name); r["model"] = json!(client.model); r["ms"] = json!(ms); r })
}

#[allow(clippy::too_many_arguments)]
fn ai_report(state: St<'_>, kind: String, page: Option<String>, prompt: String, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let settings = read_settings(settings_path(app_handle).as_deref());
    let dir = ai_dir(app_handle)?;
    let (who, picked, facts, masker, locale, page_title, heading) = {
        let mut app = lock(&state);
        gate(&app, doc_roles(app.doc.as_ref()))?;
        let picked = ai_pick(&app, &settings, provider.as_deref())?;
        let who = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        let only = if kind == "note" { page.as_deref() } else { None };
        let (facts, masker, locale) = report_facts(&mut app, only)?;
        let page_title = page.as_deref().and_then(|id| app.doc.as_ref()?.pages.iter().find(|p| p.id == id).map(|p| p.title.clone())).unwrap_or_default();
        let heading = report_heading(&mut app);
        (who, picked, facts, masker, locale, page_title, heading)
    };
    // the request belongs to the work session in front (the log ties them)
    let sid = lock(&state).work_id(true);
    let mut r = (move || {
        let lang = if lang.as_deref().unwrap_or(&settings.ui_lang) == "ru" { "ru" } else { "en" };
        ai::in_session_stamped(&sid, || ai_report_core(&who, &settings, picked, &dir, &facts, &masker, &locale, lang, &kind, &prompt, &page_title, &history.unwrap_or_default()))
    })()?;
    // which report, client and period the answer is about: a saved brief keeps them (for its PDF)
    r["heading"] = heading;
    Ok(r)
}

// ---------------------------------------------------------------- batch reports

/// A workspace of its own for one report of a batch: the same person, access and settings,
/// none of the open documents (they stay as they are).
fn batch_app(app: &App) -> App {
    App {
        doc: None, path: None, dirty: false, undo: Vec::new(), redo: Vec::new(), engine: Engine::default(), py: PyEngine::new(),
        user: app.user.clone(), notice: None, parked: Vec::new(), active_id: 0, active_link: None, next_id: 0, studio_base: None,
        settings: app.settings.clone(), session: app.session.clone(), clients: app.clients.clone(), store_path: app.store_path.clone(),
        data_params: Params::default(), data_report: None, work: Default::default(),
    }
}

/// One report of a batch, made apart from the open documents: the published report `kind`
/// ("pub:<id>") for `client` over `start`–`end`. Returns its state, for the pages to be drawn.
fn batch_make(app: &App, kind: &str, client: &str, start: Option<chrono::NaiveDate>, end: Option<chrono::NaiveDate>) -> Result<(App, Json), String> {
    let id = kind.strip_prefix("pub:").ok_or("Pick a published report.")?;
    let mut d = for_client(report_from_published(app, id)?, Some(client));
    if let (Some(s), Some(e)) = (start, end) {
        d.params.grain = "custom".into();
        d.params.start = Some(s);
        d.params.date = e;
    }
    let mut sub = load_apart(app, d, client)?;
    let st = sub.resp();
    Ok((sub, st))
}

/// `d` (made for `client`) loaded in a workspace of its own, with the person's access applied.
fn load_apart(app: &App, d: Doc, client: &str) -> Result<App, String> {
    let mut sub = batch_app(app);
    sub.load(d, None);
    sub.enforce_access();
    // a client outside the person's access is never reported on (access swaps it for another)
    let now = sub.doc.as_ref().and_then(|d| d.param_defs.iter().find(|p| auth::is_client_param(p)).and_then(|p| d.params.values.get(&p.name).cloned())).unwrap_or_default();
    if now != client {
        return Err(format!("“{client}” isn't one of your clients."));
    }
    Ok(sub)
}

/// One client's report for a batch (see `batch_make`); the open documents aren't touched.
fn batch_report(state: St<'_>, kind: String, client: String, start: Option<chrono::NaiveDate>, end: Option<chrono::NaiveDate>) -> Result<Json, String> {
    let app = lock(&state);
    gate(&app, &["user", "design"])?;
    batch_make(&app, &kind, &client, start, end).map(|(_, st)| st)
}

/// Writes one report of a batch into `folder` as `name`.pdf / .pptx (an existing file of that
/// name is replaced) and logs it as the client's report sent out of the app.
#[allow(clippy::too_many_arguments)]
fn batch_write(app: &App, folder: &str, name: &str, format: &str, pages: Json, width: f64, height: f64, client: &str, title: &str) -> Result<String, String> {
    let dir = PathBuf::from(folder);
    if !dir.is_dir() {
        return Err(format!("The folder {folder} doesn't exist."));
    }
    let ext = if format == "pptx" { "pptx" } else { "pdf" };
    let path = dir.join(format!("{}.{ext}", sanitize(name)));
    let r = (|| {
        let bytes = if ext == "pdf" {
            let svgs: Vec<String> = serde_json::from_value(pages).map_err(|e| e.to_string())?;
            export::pdf(&svgs, width, height, name)?
        } else {
            let slides: Vec<export::Slide> = serde_json::from_value(pages).map_err(|e| format!("Invalid slides: {e}"))?;
            export::pptx(&slides, width, height, name)?
        };
        std::fs::write(&path, bytes).map_err(|e| format!("Could not write {}: {e}", path.display()))
    })();
    app.audit("document", "batch export", title, &format!("Batch report · {client} · {}", path.display()), json!({"client": client, "path": path.display().to_string(), "format": ext}),
        r.as_ref().err().map(|e| e.as_str()));
    r.map(|_| path.display().to_string())
}

#[allow(clippy::too_many_arguments)]
fn batch_save(state: St<'_>, folder: String, name: String, format: String, pages: Json, width: f64, height: f64, client: String, title: String) -> Result<String, String> {
    let app = lock(&state);
    gate(&app, &["user", "design"])?;
    batch_write(&app, &folder, &name, &format, pages, width, height, &client, &title)
}

/// A folder for a batch's files.
fn pick_folder(state: St<'_>) -> Result<Option<String>, String> {
    let app_handle: H = state.host();
    let picked = app_handle.dialog().file().blocking_pick_folder()?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

/// The text of a file the banker uploads for the proposal (.docx, .pptx, .txt or .md), sent as base64.
fn proposal_read(_state: St<'_>, name: String, data: String) -> Result<String, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|_| "The file is unreadable.".to_string())?;
    proposal::extract(&name, &bytes)
}

/// The proposal written up by the AI from the banker's notes and the report's figures (both masked).
#[allow(clippy::too_many_arguments)]
fn proposal_ai_core(who: &str, settings: &Settings, picked: (&'static ai::Provider, String, &'static str), dir: &Path, facts: &str, masker: &mut ai_report::Masker,
    locale: &str, lang: &str, notes: &str) -> Result<Json, String> {
    let (p, key, src) = picked;
    let set = settings.ai.providers.get(p.id).cloned().unwrap_or_default();
    let tmp = ai::tmp_dir(dir);
    let mut client = ai::Client::new(p, &set, key, &tmp);
    client.effort = Some("medium");
    let system = proposal::system(locale, lang);
    // the notes may name the client, accounts and instruments: the same placeholders as the report's
    let masked = masker.mask(notes);
    let mut msgs = vec![ai::Msg::user(format!("THE REPORT\n{facts}\n\nTHE BANKER'S NOTES\n{masked}\n\nWrite the proposal page from the notes."))];
    let sent = system.len() + msgs[0].content.len();
    let started = std::time::Instant::now();
    let mut result: Result<Json, String> = Err("No answer.".into());
    for _ in 0..2 {
        match client.chat(&system, &msgs, 6000) {
            Ok(reply) => match ai::parse_answer(&reply) {
                Some(mut j) => {
                    masker.unmask_json(&mut j);
                    result = Ok(j);
                    break;
                }
                None => {
                    msgs.push(ai::Msg::assistant(reply));
                    msgs.push(ai::Msg::user("Answer with the JSON object only."));
                }
            },
            Err(e) => { result = Err(e); break; }
        }
    }
    ai::log_with(dir, &client, notes, json!({"at": chrono::Local::now().to_rfc3339(), "user": who, "provider": p.id, "model": client.model, "key": src, "feature": "report.proposal",
        "policy": "aggregates", "sent_chars": sent, "ok": result.is_ok(), "ms": started.elapsed().as_millis() as u64, "error": result.as_ref().err()}));
    result.map(|mut r| { r["provider"] = json!(p.name); r["model"] = json!(client.model); r })
}

/// Puts a proposal in the report — proposal `id` replaced in place, or (no id) a new one — from a
/// proposal and the notes it came from: on one page when it fits, else on two (the actions first,
/// going on over further pages if need be, then why / risks / next steps).
#[cfg_attr(not(test), allow(dead_code))]
fn proposal_apply(app: &mut App, id: Option<String>, spec: Json, notes: &str) -> Result<Json, String> {
    proposal_apply_as(app, id, spec, notes, "notes")
}

/// `proposal_apply`, recording where the proposal came from ("notes", "ai", "draft") in the register.
fn proposal_apply_as(app: &mut App, id: Option<String>, mut spec: Json, notes: &str, source: &str) -> Result<Json, String> {
    let doc = app.doc.as_ref().ok_or("Open a report first.")?;
    if doc.kind != DocKind::Report {
        return Err("A proposal goes into a report.".into());
    }
    let id = match id.filter(|i| !i.is_empty()) {
        Some(i) if proposal::list(doc).iter().any(|(x, _)| *x == i) => i,
        Some(_) => return Err("That proposal is no longer in the report.".into()),
        None => proposal::next_id(doc),
    };
    // its entry in the register: the one it has, or a new one
    let uid = proposal::uid_of(doc, &id).unwrap_or_else(proposals::new_uid);
    let ai_at = spec["log_at"].as_str().map(String::from);
    if let Some(o) = spec.as_object_mut() {
        o.insert("uid".into(), json!(uid));
    }
    let g = proposal::group(&id);
    let saved = app.doc.clone();
    let undo_len = app.undo.len();
    let edit = |split: bool| Edit::SetProposal { id: Some(id.clone()), spec: Some(spec.clone()), notes: notes.to_string(), split };
    let mut st = run_edit(app, edit(false))?;
    let cut = |st: &Json, doc: &Doc| doc.pages.iter().enumerate().filter(|(_, p)| p.group == g)
        .any(|(i, _)| st["pages"][i]["issues"].as_array().into_iter().flatten()
            .any(|x| x["severity"] != "info" && matches!(x["kind"].as_str(), Some("text_clipped" | "overflow" | "table_rows"))));
    if cut(&st, app.doc.as_ref().unwrap()) {
        // doesn't fit: the same proposal on two pages (and more for a long table), as one change
        app.doc = saved;
        app.undo.truncate(undo_len);
        st = run_edit(app, edit(true))?;
    }
    let page = app.doc.as_ref().and_then(|d| d.pages.iter().find(|p| p.group == g)).map(|p| p.id.clone()).unwrap_or_default();
    // kept in the register (a failure there doesn't undo the page)
    let entry = proposal_register(app, &uid, &spec, notes, source, ai_at.as_deref(), &g)
        .map_err(|e| eprintln!("[proposals] {e}")).ok();
    Ok(json!({"st": st, "page": page, "id": id, "uid": uid, "entry": entry}))
}

/// Records a proposal in the register with where it is: client, report, period, file, who.
fn proposal_register(app: &App, uid: &str, spec: &Json, notes: &str, source: &str, ai_at: Option<&str>, group: &str) -> Result<Json, String> {
    let dir = app.audit_dir().ok_or("No place to keep proposals.")?;
    let doc = app.doc.as_ref().ok_or("No report.")?;
    let client = doc.param_defs.iter().find(|d| auth::is_client_param(d)).and_then(|d| doc.params.values.get(&d.name)).cloned().unwrap_or_default();
    let client_id = auth::options(&app.store()).ok()
        .and_then(|o| o["clients"].as_array().and_then(|a| a.iter().find(|c| c["name"] == client.as_str()).and_then(|c| c["id"].as_str().map(String::from))))
        .unwrap_or_default();
    let (user, name) = app.session.as_ref().map(|s| (s.login.clone(), s.name.clone())).unwrap_or_default();
    let file = app.path.as_ref().map(|p| p.display().to_string());
    let cx = proposals::Context { client: &client, client_id: &client_id, report: &doc.meta.title, period: &period::label(&doc.params), file: file.as_deref(), user: &user, user_name: &name };
    let pages = doc.pages.iter().filter(|p| p.group == group).count();
    let r = proposals::record(&dir, uid, spec, notes, source, ai_at, &cx, pages)?;
    app.audit("proposal", "save", &client, &format!("Proposal «{}» · {client}", spec["title"].as_str().unwrap_or("")), json!({"uid": uid, "source": source}), None);
    Ok(r)
}

/// A report went out (exported to PDF or PowerPoint): every proposal in it is presented — a draft's
/// status becomes "presented". A proposal made before the register gets its entry first (read
/// from its pages).
fn proposals_went_out(app: &mut App, format: &str) {
    let Some(dir) = app.audit_dir() else { return };
    let Some(doc) = app.doc.as_ref() else { return };
    let missing: Vec<String> = proposal::list(doc).into_iter().map(|(id, _)| id).filter(|id| proposal::uid_of(doc, id).is_none()).collect();
    for id in missing {
        let uid = proposals::new_uid();
        let (spec, notes) = proposal::spec_of(app.doc.as_ref().unwrap(), &id);
        if let Some(d) = app.doc.as_mut() {
            proposal::set_uid(d, &id, &uid);
            app.dirty = true;
        }
        let _ = proposal_register(app, &uid, &spec, &notes, "notes", None, &proposal::group(&id));
    }
    let user = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
    let uids = app.doc.as_ref().map(proposal::uids).unwrap_or_default();
    let _ = proposals::exported(&dir, &uids, &user, &format!("Report exported to {}", format.to_uppercase()));
}

/// Takes proposal `id` out of the report; the register keeps it (marked as no longer in a report).
fn proposal_remove_run(app: &mut App, id: &str) -> Result<Json, String> {
    let uid = app.doc.as_ref().and_then(|d| proposal::uid_of(d, id));
    let st = run_edit(app, Edit::SetProposal { id: Some(id.to_string()), spec: None, notes: String::new(), split: false })?;
    if let (Some(uid), Some(dir)) = (uid, app.audit_dir()) {
        let user = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        let _ = proposals::removed(&dir, &uid, &user);
    }
    Ok(st)
}

fn proposal_remove(state: St<'_>, id: String) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, doc_roles(app.doc.as_ref()))?;
    proposal_remove_run(&mut app, &id)
}

/// The names of the clients this person may see (None: all).
fn allowed_names(app: &App) -> Option<Vec<String>> {
    if app.session.as_ref().is_some_and(|s| s.has("admin")) {
        return None;
    }
    app.clients.as_ref().map(|c| c.iter().map(|(_, n)| n.clone()).collect())
}

/// The register: a banker's clients' proposals (or one client's); an administrator's, everyone's.
fn proposals_list_run(app: &App, client: Option<&str>, all: bool) -> Result<Json, String> {
    if all {
        gate(app, &["admin"])?;
        return Ok(json!(proposals::list(app.audit_dir().as_deref(), None, client)));
    }
    gate(app, &[])?;
    let names = allowed_names(app);
    Ok(json!(proposals::list(app.audit_dir().as_deref(), names.as_deref(), client)))
}

fn proposals_list(state: St<'_>, client: Option<String>, all: Option<bool>) -> Result<Json, String> {
    proposals_list_run(&lock(&state), client.as_deref(), all.unwrap_or(false))
}

/// A proposal's status (presented, accepted…), with a note: by a banker for their clients, or an admin.
fn proposal_status_run(app: &App, uid: &str, status: &str, note: &str) -> Result<Json, String> {
    gate(app, &[])?;
    let dir = app.audit_dir().ok_or("No place to keep proposals.")?;
    let entry = proposals::get(Some(&dir), uid).ok_or("That proposal is no longer in the register.")?;
    if let Some(names) = allowed_names(app) {
        if !names.iter().any(|n| entry["place"]["client"] == n.as_str()) {
            return Err("That proposal is for a client outside your access.".into());
        }
    }
    let user = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
    let r = proposals::set_status(&dir, uid, status, note, &user)?;
    app.audit("proposal", "status", entry["place"]["client"].as_str().unwrap_or(""), &format!("Proposal «{}» · {status}", entry["content"]["title"].as_str().unwrap_or("")), json!({"uid": uid, "status": status, "note": note}), None);
    Ok(r)
}

fn proposal_status(state: St<'_>, uid: String, status: String, note: Option<String>) -> Result<Json, String> {
    proposal_status_run(&lock(&state), &uid, &status, note.as_deref().unwrap_or(""))
}

/// A drafted proposal (the report assistant's) put in the report as it is.
fn proposal_set(state: St<'_>, id: Option<String>, spec: Json, notes: String) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, doc_roles(app.doc.as_ref()))?;
    proposal_apply_as(&mut app, id, spec, &notes, "draft")
}

/// The banker's proposal page from their notes: laid out as written (`mode` "plain"), or
/// written up by the AI (`mode` "ai": needs client figures allowed, as the brief does).
fn proposal_make(state: St<'_>, id: Option<String>, notes: String, mode: String, provider: Option<String>, lang: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    if notes.trim().is_empty() {
        return Err("Write or upload the proposal first.".into());
    }
    if mode != "ai" {
        let mut app = lock(&state);
        gate(&app, doc_roles(app.doc.as_ref()))?;
        let ru = app.doc.as_ref().is_some_and(|d| d.theme.locale.starts_with("ru"));
        return proposal_apply_as(&mut app, id, proposal::plain(&notes, ru), &notes, "notes");
    }
    let settings = read_settings(settings_path(app_handle).as_deref());
    let dir = ai_dir(app_handle)?;
    let (who, picked, facts, mut masker, locale) = {
        let mut app = lock(&state);
        gate(&app, doc_roles(app.doc.as_ref()))?;
        let picked = ai_pick(&app, &settings, provider.as_deref())?;
        let who = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        // without the proposal page itself (it is being rewritten)
        let saved = app.doc.clone();
        let g = id.as_deref().map(proposal::group);
        if let (Some(d), Some(g)) = (app.doc.as_mut(), g.as_ref()) { d.pages.retain(|p| &p.group != g); }
        let r = report_facts(&mut app, None);
        app.doc = saved;
        let (facts, masker, locale) = r?;
        (who, picked, facts, masker, locale)
    };
    let sid = lock(&state).work_id(true);
    let n2 = notes.clone();
    let spec = (move || {
        let lang = if lang.as_deref().unwrap_or(&settings.ui_lang) == "ru" { "ru" } else { "en" };
        ai::in_session_stamped(&sid, || proposal_ai_core(&who, &settings, picked, &dir, &facts, &mut masker, &locale, lang, &n2))
    })()?;
    let mut app = lock(&state);
    let mut r = proposal_apply_as(&mut app, id, spec.clone(), &notes, "ai")?;
    for k in ["explanation", "provider", "model"] {
        r[k] = spec[k].clone();
    }
    Ok(r)
}

/// The open report's title, client (as the toolbar shows it) and period, for a brief's PDF.
fn report_heading(app: &mut App) -> Json {
    let st = app.resp();
    let Some(doc) = app.doc.as_ref() else { return Json::Null };
    let mut clients: Vec<String> = Vec::new();
    for d in doc.param_defs.iter().filter(|d| auth::is_client_param(d)) {
        if let Some(v) = doc.params.values.get(&d.name) {
            for x in model::param_values(v).into_iter().filter(|x| *x != "All") {
                clients.push(st["param_labels"][&d.name][x].as_str().map(String::from).unwrap_or_else(|| x.to_string()));
            }
        }
    }
    json!({"title": doc.meta.title, "client": clients.join(", "), "period": period::label(&doc.params), "locale": doc.theme.locale})
}

/// Saves a brief (pages drawn as SVG) as a PDF where the user picks.
fn brief_pdf(state: St<'_>, pages: Vec<String>, width: f64, height: f64, name: String) -> Result<Option<String>, String> {
    let app_handle: H = state.host();
    let picked = app_handle.dialog().file().set_file_name(format!("{}.pdf", sanitize(&name))).add_filter("PDF", &["pdf"]).blocking_save_file()?;
    let Some(fp) = picked else { return Ok(None) };
    let mut path = fp.into_path().map_err(|e| e.to_string())?;
    if path.extension().is_none_or(|e| e != "pdf") {
        path.set_extension("pdf");
    }
    let r = export::pdf(&pages, width, height, &name).and_then(|b| std::fs::write(&path, b).map_err(|e| format!("Could not write {}: {e}", path.display())));
    lock(&state).audit("document", "export brief", "", &format!("Brief saved as PDF · {}", path.display()), Json::Null, r.as_ref().err().map(|e| e.as_str()));
    r.map(|_| Some(path.display().to_string()))
}

/// The designer's assistant (see `ai_design_core`): `kind` "page" builds a new page after `page`,
/// "change" changes `page`. The model is asked with the app unlocked.
#[allow(clippy::too_many_arguments)]
fn ai_design(state: St<'_>, kind: String, prompt: String, page: String, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let settings = read_settings(settings_path(app_handle).as_deref());
    let dir = ai_dir(app_handle)?;
    let (who, picked, ctx) = {
        let app = lock(&state);
        gate(&app, &["design"])?;
        let doc = app.doc.as_ref().ok_or("Open a template first.")?;
        if doc.kind != DocKind::Template {
            return Err("The design assistant works on templates.".into());
        }
        let picked = ai_pick(&app, &settings, provider.as_deref())?;
        let who = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        let target = doc.pages.iter().find(|p| p.id == page);
        (who, picked, ai_design::context(doc, if kind == "change" { target } else { None }))
    };
    // the request belongs to the work session in front (the log ties them)
    let sid = lock(&state).work_id(true);
    (move || {
        let lang = if lang.as_deref().unwrap_or(&settings.ui_lang) == "ru" { "ru" } else { "en" };
        let apply = |ans: &Json, last: bool| -> Result<Json, Vec<String>> {
            let mut app = lock(state);
            if kind == "change" { design_apply(&mut app, Some(&page), None, ans, last) } else { design_apply(&mut app, None, Some(&page), ans, last) }
        };
        ai::in_session_stamped(&sid, || ai_design_core(&who, &settings, picked, &dir, &ctx, lang, &kind, &prompt, &history.unwrap_or_default(), apply))
    })()
}

fn ai_status(state: St<'_>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let settings = read_settings(settings_path(app_handle).as_deref());
    ai_status_of(&lock(&state), &settings)
}

/// Whose key a change is for: the person's own; for an administrator also the bank's (`bank`)
/// or another user's (`user`).
fn ai_key_owner(app: &App, bank: bool, user: Option<&str>) -> Result<String, String> {
    let s = app.session.as_ref().ok_or("Sign in first.")?;
    if bank {
        if !s.has("admin") {
            return Err("Only an administrator sets the bank’s keys.".into());
        }
        return Ok(ai::org().to_string());
    }
    match user.map(str::trim).filter(|u| !u.is_empty() && !u.eq_ignore_ascii_case(&s.login)) {
        None => Ok(s.login.clone()),
        Some(u) => {
            if !s.has("admin") {
                return Err("Only an administrator sets other people’s keys.".into());
            }
            let st = app.store();
            let a = st.users.iter().find(|a| a.login.eq_ignore_ascii_case(u)).ok_or("No such user.")?;
            Ok(a.login.clone())
        }
    }
}

/// Saves (or with no key, removes) an API key: the person's own, or — for an administrator —
/// the bank's or another user's.
/// `ai_set_key`, recorded in the activity log.
fn ai_set_key(state: St<'_>, provider: String, key: Option<String>, bank: bool, user: Option<String>) -> Result<Json, String> {
    let (pv, removed, whose) = (provider.clone(), key.as_deref().map(|k| k.trim().is_empty()).unwrap_or(true), if bank { "the bank’s".to_string() } else { user.clone().filter(|u| !u.is_empty()).map(|u| format!("{u}’s")).unwrap_or_else(|| "own".into()) });
    let r = ai_set_key_run(state, provider, key, bank, user);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("ai", if removed { "remove key" } else { "save key" }, &pv, &format!("{} {whose} {pv} key", if removed { "Removed" } else { "Saved" }), Json::Null, err);
    }
    r
}

fn ai_set_key_run(state: St<'_>, provider: String, key: Option<String>, bank: bool, user: Option<String>) -> Result<Json, String> {
    let app = lock(&state);
    if ai::provider(&provider).is_none() {
        return Err("Unknown provider.".into());
    }
    let who = ai_key_owner(&app, bank, user.as_deref())?;
    ai::key_set(&provider, &who, key.as_deref().filter(|k| !k.trim().is_empty()))?;
    Ok(json!({"saved": true}))
}

/// For the Admin area: which providers a user has a key of their own for (never the keys).
fn ai_user_keys_of(app: &App, login: &str) -> Result<Json, String> {
    gate(app, &["admin"])?;
    let who = ai_key_owner(app, false, Some(login))?;
    let policy = app.store().ai;
    Ok(json!(ai::PROVIDERS.iter().map(|p| json!({"id": p.id, "name": p.name, "keys_at": p.keys_at, "gigachat": p.id == "gigachat",
        "personal": ai::key_has(p.id, &who), "bank": ai::key_has(p.id, ai::org()), "allowed": policy.allows(p.id)})).collect::<Vec<_>>()))
}

fn ai_user_keys(state: St<'_>, login: String) -> Result<Json, String> {
    ai_user_keys_of(&lock(&state), &login)
}

/// Checks a provider with its key and lists the models it offers.
fn ai_test(state: St<'_>, provider: String, settings: ai::ProviderSettings) -> Result<Json, String> {
    let app_handle: H = state.host();
    let (p, key, src) = {
        let app = lock(&state);
        let s = app.session.as_ref().ok_or("Sign in first.")?;
        let p = ai::provider(&provider).ok_or("Unknown provider.")?;
        let (k, src) = ai::key_for(p.id, &s.login).ok_or("Add a key first.")?;
        (p, k, src)
    };
    let dir = ai_dir(app_handle)?;
    (move || {
        let tmp = ai::tmp_dir(&dir);
        let c = ai::Client::new(p, &settings, key, &tmp);
        let models = c.models()?;
        Ok(json!({"ok": true, "models": models, "model": c.model, "key": src}))
    })()
}

/// `ai_set_policy`, recorded in the activity log.
fn ai_set_policy(state: St<'_>, policy: ai::AiPolicy) -> Result<Json, String> {
    let pol = serde_json::to_value(&policy).unwrap_or(Json::Null);
    let r = ai_set_policy_run(state, policy);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("ai", "bank policy", "", &format!("Data that may be sent: {}{}{}", pol["level"].as_str().unwrap_or(""), if pol["off"] == true { " · AI off" } else { "" }, pol["providers"].as_array().filter(|a| !a.is_empty()).map(|a| format!(" · providers: {}", a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "))).unwrap_or_default()), pol, err);
    }
    r
}

fn ai_set_policy_run(state: St<'_>, policy: ai::AiPolicy) -> Result<Json, String> {
    let app = lock(&state);
    gate(&app, &["admin"])?;
    let mut st = app.store();
    // a policy saved without prices keeps the ones set before
    let prices = if policy.prices.is_empty() { st.ai.prices.clone() } else { policy.prices.clone() };
    st.ai = ai::AiPolicy { prices, ..policy };
    app.save_store(&st)?;
    Ok(json!({"saved": true}))
}

fn ai_log(state: St<'_>) -> Result<Json, String> {
    let app_handle: H = state.host();
    gate(&lock(&state), &["admin"])?;
    Ok(json!(ai::read_log(&ai_dir(app_handle)?, 300)))
}

/// The activity log for the Admin area: what people did, the AI requests among it, filtered.
fn activity(state: St<'_>, filter: Option<audit::Filter>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let dir = {
        let app = lock(&state);
        gate(&app, &["admin"])?;
        app.audit_dir()
    };
    let ai = ai_dir(app_handle).ok();
    Ok(audit::read(dir.as_deref(), ai.as_deref(), &filter.unwrap_or_default()))
}

/// The administrator's assistant: `kind` "ask" (a question about the activity), "explain" (a failed
/// event, `at` + `of`), "access" (access set up in words: a proposal, applied by the UI after a
/// confirmation) or "review" (who can do what, and what looks wrong). People, bankers and clients
/// go to the model as placeholders.
#[allow(clippy::too_many_arguments)]
fn ai_admin(state: St<'_>, kind: String, prompt: String, at: Option<String>, of: Option<String>, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let settings = read_settings(settings_path(app_handle).as_deref());
    let dir = ai_dir(app_handle)?;
    let (who, picked, store, adir) = {
        let app = lock(&state);
        gate(&app, &["admin"])?;
        let picked = ai_pick(&app, &settings, provider.as_deref())?;
        (app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default(), picked, app.store(), app.audit_dir())
    };
    let lang = if lang.as_deref().unwrap_or(&settings.ui_lang) == "ru" { "ru" } else { "en" }.to_string();
    let sid = lock(&state).admin_work_id();
    (move || ai::in_session_stamped(&sid, || ai_admin_run(&who, &settings, picked, &dir, adir.as_deref(), &store, &kind, &prompt, at.as_deref(), of.as_deref(), &history.unwrap_or_default(), &lang)))()
}

#[allow(clippy::too_many_arguments)]
fn ai_admin_run(who: &str, settings: &Settings, picked: (&'static ai::Provider, String, &'static str), dir: &Path, adir: Option<&Path>, store: &auth::Store,
    kind: &str, prompt: &str, at: Option<&str>, of: Option<&str>, history: &[ai::Msg], lang: &str) -> Result<Json, String> {
    let opts = auth::options(store).unwrap_or(Json::Null);
    let since = (chrono::Local::now() - chrono::Duration::days(30)).format("%Y-%m-%d").to_string();
    // explaining a failure: that person's own activity, and the event itself in full
    let person = if kind == "explain" { of.unwrap_or("").to_string() } else { String::new() };
    let f = audit::Filter { from: since, user: person.clone(), limit: 400, ..Default::default() };
    let sessions = audit::sessions(adir, Some(dir), &f);
    let events = audit::read(adir, Some(dir), &f);
    let (text, mut masker, sids, eids) = if kind == "access" {
        let (t, m) = ai_admin::access_ctx(store, &opts);
        (t, m, Vec::new(), Vec::new())
    } else {
        let extra = match kind {
            "review" => ai_admin::review_facts(store, &sessions, &events),
            "explain" => {
                let ev = events["rows"].as_array().into_iter().flatten().find(|e| e["at"].as_str() == at).cloned().unwrap_or(Json::Null);
                if ev.is_null() {
                    return Err("That event is no longer in the log.".into());
                }
                let data: String = ev["data"].to_string().chars().take(3000).collect();
                format!("FAILED EVENT: {} · {{{{who:{}}}}} · {} · {} · {} · error: {}
its data: {data}", ev["at"].as_str().unwrap_or("").replace('T', " "), ev["user"].as_str().unwrap_or(""),
                    ev["area"].as_str().unwrap_or(""), ev["action"].as_str().unwrap_or(""), ev["detail"].as_str().unwrap_or(""), ev["error"].as_str().unwrap_or(""))
            }
            _ => String::new(),
        };
        let c = ai_admin::activity(store, &opts, &sessions, &events, &extra);
        (c.text, c.masker, c.sessions, c.events)
    };
    let (p, key, src) = picked;
    let set = settings.ai.providers.get(p.id).cloned().unwrap_or_default();
    let tmp = ai::tmp_dir(dir);
    let mut client = ai::Client::new(p, &set, key, &tmp);
    client.effort = Some(if kind == "access" { "medium" } else { "low" });
    let system = ai_admin::system(kind, lang);
    let mut msgs = ai::tidy_history(history);
    let ask = match kind {
        "review" => "Review the access and the activity.".to_string(),
        "explain" => "Explain the failed event.".to_string(),
        _ => masker.mask(prompt),
    };
    msgs.push(ai::Msg::user(format!("{text}

{ask}")));
    let sent = system.len() + msgs.iter().map(|m| m.content.len()).sum::<usize>();
    let started = std::time::Instant::now();
    let mut result: Result<Json, String> = Err("No answer.".into());
    for _ in 0..2 {
        match client.chat(&system, &msgs, 8000) {
            Ok(reply) => match ai::parse_answer(&reply) {
                Some(mut j) => {
                    masker.unmask_json(&mut j);
                    result = Ok(j);
                    break;
                }
                None => {
                    msgs.push(ai::Msg::assistant(reply));
                    msgs.push(ai::Msg::user("Answer with the JSON object only."));
                }
            },
            Err(e) => { result = Err(e); break; }
        }
    }
    ai::log_with(dir, &client, prompt, json!({"at": chrono::Local::now().to_rfc3339(), "user": who, "provider": p.id, "model": client.model, "key": src, "feature": format!("admin.{kind}"),
        "policy": "structure", "sent_chars": sent, "ok": result.is_ok(), "ms": started.elapsed().as_millis() as u64, "error": result.as_ref().err()}));
    let ms = started.elapsed().as_millis() as u64;
    let mut r = result?;
    // what the answer rests on, as links: S3 → that session, E17 → that event
    let link = |x: &str| -> Option<Json> {
        let n: usize = x.get(1..)?.parse().ok()?;
        match x.chars().next()? {
            'S' => sids.get(n - 1).map(|id| json!({"type": "session", "id": id, "label": x})),
            'E' => eids.get(n - 1).map(|(at, user)| json!({"type": "event", "at": at, "user": user, "label": x})),
            _ => None,
        }
    };
    let links = |v: &Json| -> Json { json!(v.as_array().into_iter().flatten().filter_map(|x| x.as_str()).filter_map(link).collect::<Vec<_>>()) };
    r["links"] = links(&r["refs"]);
    if let Some(fs) = r["findings"].as_array_mut() {
        for f in fs.iter_mut() {
            let l = links(&f["refs"]);
            f["links"] = l;
            // the person a finding is about, by login (the answer names them)
            if let Some(p) = f["person"].as_str() {
                if let Some(u) = store.users.iter().find(|u| u.name == p || u.login == p) { f["login"] = json!(u.login); }
            }
        }
    }
    if kind == "access" {
        r["plan"] = ai_admin::resolve_access(store, &opts, &r);
    }
    r["provider"] = json!(p.name);
    r["model"] = json!(client.model);
    r["ms"] = json!(ms);
    Ok(r)
}

/// Work sessions (Admin → Activity → Sessions): one per document and client, with its context,
/// time and what was done in it.
fn work_sessions(state: St<'_>, filter: Option<audit::Filter>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let dir = {
        let app = lock(&state);
        gate(&app, &["admin"])?;
        app.audit_dir()
    };
    let ai = ai_dir(app_handle).ok();
    // the events from before sessions existed get theirs, once
    if let Some(d) = dir.as_deref() { audit::rebuild_once(d, ai.as_deref()); }
    Ok(audit::sessions(dir.as_deref(), ai.as_deref(), &filter.unwrap_or_default()))
}

/// One logged request in full: what the person asked, what was sent, what came back.
fn ai_log_entry(state: St<'_>, at: String, user: String) -> Result<Json, String> {
    let app_handle: H = state.host();
    gate(&lock(&state), &["admin"])?;
    let dir = ai_dir(app_handle)?;
    let mut e = ai::read_log_entry(&dir, &at, &user).ok_or("That request is no longer in the log.")?;
    // what the person said about the answer
    e["feedback"] = audit::feedback(Some(&dir)).remove(&(at, user)).unwrap_or(Json::Null);
    Ok(e)
}

/// A person's thumbs up or down on an AI answer (`at`: when its request was logged), with what went
/// wrong when it didn't help: kept for administrators (Admin → AI and Activity).
fn ai_feedback(state: St<'_>, at: String, vote: String, reasons: Option<Vec<String>>, comment: Option<String>) -> Result<Json, String> {
    let app_handle: H = state.host();
    let dir = ai_dir(app_handle)?;
    let app = lock(&state);
    ai_feedback_run(&app, &dir, &at, &vote, &reasons.unwrap_or_default(), &comment.unwrap_or_default())
}

fn ai_feedback_run(app: &App, dir: &Path, at: &str, vote: &str, reasons: &[String], comment: &str) -> Result<Json, String> {
    let user = app.session.as_ref().map(|s| s.login.clone()).ok_or("Sign in first.")?;
    if !matches!(vote, "up" | "down" | "") {
        return Err("A vote is up or down.".into());
    }
    // only one's own requests
    let entry = ai::read_log_entry(dir, at, &user).ok_or("That request isn't in the log.")?;
    let feature = entry["feature"].as_str().unwrap_or("").to_string();
    audit::record_feedback(dir, &user, at, &feature, vote, reasons, comment)?;
    let words = match vote { "up" => "Helpful".to_string(), "down" => format!("Not helpful{}", if reasons.is_empty() { String::new() } else { format!(" · {}", reasons.join(", ")) }), _ => "Vote taken back".into() };
    app.audit("ai", "feedback", &feature, &format!("{words}{}", if comment.trim().is_empty() { String::new() } else { format!(" · “{}”", comment.trim()) }),
        json!({"request_at": at, "vote": vote, "reasons": reasons, "comment": comment}), None);
    Ok(json!({"saved": true}))
}

/// The SQL workspace's assistant (see `ai_sql_core`). The model is asked with the app unlocked;
/// each query it writes runs like one from the editor.
#[allow(clippy::too_many_arguments)]
fn ai_sql(state: St<'_>, connection: model::Connection, kind: String, prompt: String, sql: String, error: String, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>) -> Result<Json, String> {
    let connection = real_conn(&lock(state), connection);
    let app_handle: H = state.host();
    let settings = read_settings(settings_path(app_handle).as_deref());
    let dir = ai_dir(app_handle)?;
    let (who, picked, tables, params) = {
        let mut app = lock(&state);
        gate(&app, &["data", "design"])?;
        let picked = ai_pick(&app, &settings, provider.as_deref())?;
        let who = app.session.as_ref().map(|s| s.login.clone()).unwrap_or_default();
        let base = app.base_dir();
        let tables = dbconn::tables(&connection, base.as_deref(), &mut app.py)?;
        app.resolve_lookups();
        let params = dbconn::params(&preview_doc(&app));
        (who, picked, tables, params)
    };
    // the request belongs to the work session in front (the log ties them)
    let sid = lock(&state).work_id(true);
    (move || {
        // the language the app shows now (else the saved one)
        let lang = if lang.as_deref().unwrap_or(&settings.ui_lang) == "ru" { "ru" } else { "en" };
        let run = |q: &str| -> Result<Json, String> {
            let mut app = lock(state);
            app.resolve_lookups();
            let base = app.base_dir();
            let doc = preview_doc(&app);
            dbconn::run(&connection, q, 5, Some(&doc), base.as_deref(), &mut app.py)
        };
        ai::in_session_stamped(&sid, || ai_sql_core(&who, &settings, picked, &dir, &connection.kind, &tables, &params, lang, &kind, &prompt, &sql, &error, &history.unwrap_or_default(), run))
    })()
}

/// A connection's tables and views, with their columns and row counts.
fn db_tables(state: St<'_>, connection: model::Connection) -> Result<Json, String> {
    let connection = real_conn(&lock(state), connection);
    let mut app = lock(&state);
    gate(&app, &["data", "design"])?;
    let base = app.base_dir();
    dbconn::tables(&connection, base.as_deref(), &mut app.py)
}

/// Runs SQL on a connection (read-only) and returns the first rows.
/// `db_run`, recorded in the activity log.
fn db_run(state: St<'_>, connection: model::Connection, sql: String, limit: Option<usize>) -> Result<Json, String> {
    let connection = real_conn(&lock(state), connection);
    let (cn, q) = (connection.name.clone(), sql.clone());
    let r = db_run_run(state, connection, sql, limit);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("data", "run sql", &cn, &format!("{}{}", q.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect::<String>(), r.as_ref().ok().and_then(|x| x["total"].as_u64()).map(|n| format!(" · {n} rows")).unwrap_or_default()), json!({"sql": q}), err);
    }
    r
}

fn db_run_run(state: St<'_>, connection: model::Connection, sql: String, limit: Option<usize>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["data", "design"])?;
    app.resolve_lookups();
    let base = app.base_dir();
    let doc = preview_doc(&app);
    dbconn::run(&connection, &sql, limit.unwrap_or(200).min(10_000), Some(&doc), base.as_deref(), &mut app.py)
}

/// Writes edited cells to a table of a connection (all or nothing), then
/// reloads the report's data so it shows the new values.
/// `db_apply`, recorded in the activity log.
fn db_apply(state: St<'_>, connection: model::Connection, schema: String, table: String, edits: Vec<dbconn::CellEdit>) -> Result<Json, String> {
    let connection = real_conn(&lock(state), connection);
    let (tbl, n, ed) = (format!("{}.{}", schema, table), edits.len(), serde_json::to_value(&edits).unwrap_or(Json::Null));
    let r = db_apply_run(state, connection, schema, table, edits);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        app.audit("data", "write to database", &tbl, &format!("{n} value(s) written to {tbl}"), ed, err);
    }
    r
}

fn db_apply_run(state: St<'_>, connection: model::Connection, schema: String, table: String, edits: Vec<dbconn::CellEdit>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["data"])?;
    let base = app.base_dir();
    let n = dbconn::apply(&connection, &schema, &table, &edits, base.as_deref())?;
    app.engine.clear();
    Ok(json!({"saved": n}))
}

/// The `:names` SQL can use and their current values.
fn db_params(state: St<'_>) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["data", "design"])?;
    app.resolve_lookups();
    Ok(dbconn::params(&preview_doc(&app)))
}

/// What a repeat would produce, for the designer's dialog: the source's
/// columns, each column's distinct values (up to 60), and the copies — one
/// label per matching row.
fn repeat_preview(state: St<'_>, repeat: model::Repeat) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["design"])?;
    repeat_preview_of(&mut app, &repeat)
}

fn repeat_preview_of(app: &mut App, rep: &model::Repeat) -> Result<Json, String> {
    app.resolve_lookups();
    let base = app.base_dir();
    let App { doc, engine, py, .. } = app;
    let doc = doc.as_ref().ok_or("No document is open.")?;
    let ds = doc.dataset(&rep.dataset).cloned().ok_or("Choose a data source.")?;
    let rows = engine.rows_text(doc, &ds, base.as_deref(), py)?;
    let columns: Vec<String> = rows.first().map(|r| r.keys().cloned().collect()).unwrap_or_default();
    let mut values = serde_json::Map::new();
    for c in &columns {
        let mut seen: Vec<String> = Vec::new();
        for r in &rows {
            if let Some(v) = r.get(c) {
                if !seen.contains(v) && seen.len() < 60 {
                    seen.push(v.clone());
                }
            }
        }
        values.insert(c.clone(), json!(seen));
    }
    let matching: Vec<&std::collections::BTreeMap<String, String>> = rows
        .iter()
        .filter(|r| rep.field.is_empty() || rep.values.is_empty() || r.get(&rep.field).is_some_and(|v| rep.values.iter().any(|x| x == v)))
        .collect();
    let labels: Vec<String> = matching.iter().map(|r| r.get(&rep.label).cloned().unwrap_or_else(|| r.values().next().cloned().unwrap_or_default())).collect();
    Ok(json!({"columns": columns, "values": values, "rows": rows.len(), "copies": labels, "first": matching.first()}))
}

/// Saves an SQL script: to `path`, or where the user picks (`as_new`, or no path yet).
fn sql_file_save(state: St<'_>, text: String, path: Option<String>, name: Option<String>, ext: Option<String>) -> Result<Option<String>, String> {
    let app_handle: H = state.host();
    // an SQL script, or a Python one (the workspace's Python)
    let py = ext.as_deref() == Some("py");
    let (ext, what) = if py { ("py", "Python script") } else { ("sql", "SQL script") };
    let path = match path.filter(|p| !p.is_empty()) {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let file = format!("{}.{ext}", sanitize(name.as_deref().unwrap_or("query")));
            let Some(fp) = app_handle.dialog().file().set_file_name(file).add_filter(what, &[ext]).blocking_save_file()? else { return Ok(None) };
            let mut p = fp.into_path().map_err(|e| e.to_string())?;
            if p.extension().is_none() {
                p.set_extension(ext);
            }
            p
        }
    };
    std::fs::write(&path, text).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// The window is in front and someone is working: `seconds` of active time for the document in front.
fn activity_ping(state: St<'_>, seconds: f64, area: Option<String>) -> Result<(), String> {
    lock(&state).work_time(seconds, area.as_deref().unwrap_or(""));
    Ok(())
}

/// Saves text where the user picks (the activity log as CSV…): the path, or None if cancelled.
fn save_text_file(state: St<'_>, text: String, name: String, ext: String, what: String) -> Result<Option<String>, String> {
    let app_handle: H = state.host();
    let ext = if ext.chars().all(|c| c.is_ascii_alphanumeric()) && !ext.is_empty() { ext } else { "txt".into() };
    let file = format!("{}.{ext}", sanitize(&name));
    let Some(fp) = app_handle.dialog().file().set_file_name(file).add_filter(&what, &[ext.as_str()]).blocking_save_file()? else { return Ok(None) };
    let mut p = fp.into_path().map_err(|e| e.to_string())?;
    if p.extension().is_none() {
        p.set_extension(&ext);
    }
    // a BOM so Excel reads Cyrillic in a CSV right
    let body = if ext == "csv" { format!("\u{feff}{text}") } else { text };
    std::fs::write(&p, body).map_err(|e| format!("Could not save {}: {e}", p.display()))?;
    Ok(Some(p.display().to_string()))
}

/// Opens an SQL script the user picks: its path and text.
fn sql_file_open(state: St<'_>, ext: Option<String>) -> Result<Option<Json>, String> {
    let app_handle: H = state.host();
    let py = ext.as_deref() == Some("py");
    let Some(fp) = app_handle.dialog().file().add_filter(if py { "Python script" } else { "SQL script" }, if py { &["py", "txt"][..] } else { &["sql", "txt"][..] }).blocking_pick_file()? else { return Ok(None) };
    let p = fp.into_path().map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(&p).map_err(|e| format!("Could not read {}: {e}", p.display()))?;
    Ok(Some(json!({"path": p.display().to_string(), "text": text})))
}

/// The bundled demo database, if present.
fn db_demo_path(_state: St<'_>) -> Result<Json, String> {
    Ok(json!(dbconn::demo_path()))
}

/// Daily (or monthly) balances repeat the same set of rows on every date;
/// transactions don't. A steady row count per date suggests snapshots.
fn looks_like_snapshots(t: &data::Table, date_field: Option<&str>) -> bool {
    let Some(c) = date_field.and_then(|f| t.col(f)) else { return false };
    let mut per: std::collections::BTreeMap<chrono::NaiveDate, usize> = Default::default();
    for r in &t.rows {
        if let Some(d) = r.get(c).and_then(|v| v.date()) {
            *per.entry(d).or_default() += 1;
        }
    }
    if per.len() < 3 {
        return false;
    }
    let n = per.len() as f64;
    let mean = per.values().sum::<usize>() as f64 / n;
    let var = per.values().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / n;
    mean >= 2.0 && var.sqrt() / mean < 0.05
}

fn list_fonts(_state: St<'_>) -> Result<Vec<fonts::FamilyInfo>, String> {
    Ok(fonts::families())
}

/// The document's embedded font files, for the UI to register.
fn font_files(state: St<'_>) -> Result<Json, String> {
    let app = lock(&state);
    Ok(json!(app.doc.as_ref().map(|d| d.fonts.clone()).unwrap_or_default()))
}

/// Every value a dataset's field takes (for a table column's per-value settings).
fn field_values_on(app: &mut App, dataset: &str, field: &str) -> Result<Json, String> {
    let base = app.base_dir();
    let App { doc, engine, py, .. } = app;
    let doc = doc.as_ref().ok_or("No document is open.")?;
    Ok(json!(engine.field_values(doc, dataset, field, base.as_deref(), py)))
}

fn field_values(state: St<'_>, dataset: String, field: String) -> Result<Json, String> {
    field_values_on(&mut lock(&state), &dataset, &field)
}

fn python_info(state: St<'_>) -> Result<Json, String> {
    Ok(lock(&state).py.info())
}

/// The Pythons on this computer (for Settings → Python).
fn python_envs(_state: St<'_>) -> Result<Json, String> {
    python::check()?;
    Ok(json!(python::environments()))
}

/// Python on the web server: whether it is guarded there (`server`) and switched on (`on`). An
/// administrator switches it with `on`; kept in the accounts store, for every workspace at once.
fn python_policy(state: St<'_>, on: Option<bool>) -> Result<Json, String> {
    let app = lock(state);
    gate(&app, &[])?;
    if let Some(on) = on {
        gate(&app, &["admin"])?;
        if !python::guarded() {
            return Err("Python is always available in the desktop app.".into());
        }
        let mut st = app.store();
        st.python_on_server = on;
        app.save_store(&st)?;
        python::allow(on);
        app.audit("admin", "python", "", if on { "Switched Python on for the server" } else { "Switched Python off for the server" }, json!({"on": on}), None);
    }
    Ok(json!({"server": python::guarded(), "on": python::check().is_ok()}))
}

/// Checks one interpreter: version and pandas.
fn python_test(_state: St<'_>, path: String) -> Result<Json, String> {
    python::check()?;
    python::probe(&path).ok_or_else(|| format!("{path} doesn't run as Python 3."))
}

/// Runs Python on a table (the SQL workspace's last result, as `df`).
fn py_run(state: St<'_>, code: String, data: Json) -> Result<Json, String> {
    let mut app = lock(&state);
    gate(&app, &["data", "design"])?;
    py_run_on(&mut app, &code, &data)
}

fn py_run_on(app: &mut App, code: &str, data: &Json) -> Result<Json, String> {
    let params = json!(query::sql_values(&preview_doc(app)));
    let r = app.py.run_on(code, data, &params);
    Ok(json!({ "ok": r.ok, "value": r.value, "stdout": r.stdout, "error": r.error, "ms": r.ms }))
}

// ------------------------------------------------------------------ export

/// `export`, recorded in the activity log.
fn export(state: St<'_>, format: String, pages: Json) -> Result<Option<String>, String> {
    let app_handle: H = state.host();
    let f = format.clone();
    let r = export_run(app_handle, state, format, pages);
    {
        let app = lock(&state);
        let err = r.as_ref().err().map(|e| e.as_str());
        if r.as_ref().map(|x| x.is_some()).unwrap_or(true) { app.audit("document", "export", "", &format!("Exported to {}{}", f.to_uppercase(), r.as_ref().ok().and_then(|x| x.clone()).map(|p| format!(" · {p}")).unwrap_or_default()), Json::Null, err); }
    }
    // the proposals in it went out with it: a draft becomes "presented"
    if matches!(r, Ok(Some(_))) {
        proposals_went_out(&mut lock(&state), &f);
    }
    r
}

fn export_run(app_handle: H, state: St<'_>, format: String, pages: Json) -> Result<Option<String>, String> {
    let (title, w, h) = {
        let app = lock(&state);
        let doc = app.doc.as_ref().ok_or("No document is open.")?;
        let (w, h) = doc.page.dims();
        let t = format!("{} — {}", doc.meta.title, period::label(&doc.params));
        (t, w, h)
    };
    let ext = if format == "pptx" { "pptx" } else { "pdf" };
    let picked = app_handle
        .dialog()
        .file()
        .set_file_name(format!("{}.{ext}", sanitize(&title)))
        .add_filter(if ext == "pdf" { "PDF" } else { "PowerPoint" }, &[ext])
        .blocking_save_file()?;
    let Some(fp) = picked else { return Ok(None) };
    let mut path = fp.into_path().map_err(|e| e.to_string())?;
    if path.extension().is_none_or(|e| e != ext) {
        path.set_extension(ext);
    }
    let bytes = if ext == "pdf" {
        let svgs: Vec<String> = serde_json::from_value(pages).map_err(|e| e.to_string())?;
        export::pdf(&svgs, w, h, &title)?
    } else {
        let slides: Vec<export::Slide> = serde_json::from_value(pages).map_err(|e| format!("Invalid slides: {e}"))?;
        export::pptx(&slides, w, h, &title)?
    };
    std::fs::write(&path, bytes).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

fn reveal(state: St<'_>, path: String, open: bool) -> Result<(), String> {
    state.host().reveal(Path::new(&path), open)
}

fn js_log(_state: St<'_>, level: String, msg: String) -> Result<(), String> {
    eprintln!("[ui {level}] {msg}");
    Ok(())
}

// ------------------------------------------------------------------ the command layer

/// A command's argument by its name: snake_case, or camelCase as the desktop UI sends it.
fn arg<T: serde::de::DeserializeOwned>(args: &Json, name: &str) -> Result<T, String> {
    let camel: String = name.split('_').enumerate().map(|(i, w)| if i == 0 { w.to_string() } else { w[..1].to_uppercase() + &w[1..] }).collect();
    let v = args.get(name).or_else(|| args.get(&camel)).cloned().unwrap_or(Json::Null);
    serde_json::from_value(v).map_err(|e| format!("Bad “{name}” for this request: {e}"))
}

fn out<T: Serialize>(r: Result<T, String>) -> Result<Json, String> {
    r.and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}

macro_rules! commands {
    ($( $name:ident ( $( $p:ident : $t:ty ),* ) ),* $(,)?) => {
        /// Every command the UI can send, by name.
        pub const COMMANDS: &[&str] = &[$(stringify!($name)),*];

        /// Runs one command from the UI — the desktop app's and the web server's alike. Who may
        /// do what is checked inside each command (`gate`). It blocks: run it off the UI thread.
        pub fn dispatch(state: &AppState, cmd: &str, args: &Json) -> Result<Json, String> {
            match cmd {
                $( stringify!($name) => out($name(state, $( arg::<$t>(args, stringify!($p))? ),*)), )*
                _ => Err(format!("Unknown command “{cmd}”.")),
            }
        }
    };
}

commands! {
    pick_screen_color(),
    auth(cmd: String, args: Option<Json>),
    edit(op: Edit),
    undo(),
    redo(),
    get_state(),
    refresh(),
    get_settings(),
    save_settings(settings: Settings),
    open_devtools(),
    new_doc(kind: String, client: Option<String>),
    list_themes(),
    theme_library_op(op: String, id: String, name: Option<String>),
    theme_studio_open(source: String),
    doc_activate(id: u64),
    doc_close(id: u64),
    theme_studio_swap(),
    theme_studio_close(apply: bool),
    apply_theme(id: String),
    open_path(path: String),
    open_dialog(),
    report_from_template(current: bool),
    save(save_as: bool),
    pick_data_file(kind: String),
    upload_file(name: String, data: String),
    saved_file(path: String),
    saved_documents(),
    delete_saved(path: String),
    read_image(path: String),
    excel_sheets(path: String),
    preview_dataset(dataset: Dataset, keep: Option<bool>),
    ai_pictures(data: String),
    ai_restyle(images: Vec<ai::Image>, prompt: String, facts: Option<String>, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>),
    ai_report(kind: String, page: Option<String>, prompt: String, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>),
    batch_report(kind: String, client: String, start: Option<chrono::NaiveDate>, end: Option<chrono::NaiveDate>),
    batch_save(folder: String, name: String, format: String, pages: Json, width: f64, height: f64, client: String, title: String),
    pick_folder(),
    proposal_read(name: String, data: String),
    proposal_remove(id: String),
    proposals_list(client: Option<String>, all: Option<bool>),
    proposal_status(uid: String, status: String, note: Option<String>),
    proposal_set(id: Option<String>, spec: Json, notes: String),
    proposal_make(id: Option<String>, notes: String, mode: String, provider: Option<String>, lang: Option<String>),
    brief_pdf(pages: Vec<String>, width: f64, height: f64, name: String),
    ai_design(kind: String, prompt: String, page: String, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>),
    ai_status(),
    ai_set_key(provider: String, key: Option<String>, bank: bool, user: Option<String>),
    ai_user_keys(login: String),
    ai_test(provider: String, settings: ai::ProviderSettings),
    ai_set_policy(policy: ai::AiPolicy),
    ai_log(),
    activity(filter: Option<audit::Filter>),
    ai_admin(kind: String, prompt: String, at: Option<String>, of: Option<String>, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>),
    work_sessions(filter: Option<audit::Filter>),
    ai_log_entry(at: String, user: String),
    ai_feedback(at: String, vote: String, reasons: Option<Vec<String>>, comment: Option<String>),
    ai_sql(connection: model::Connection, kind: String, prompt: String, sql: String, error: String, provider: Option<String>, history: Option<Vec<ai::Msg>>, lang: Option<String>),
    db_tables(connection: model::Connection),
    db_run(connection: model::Connection, sql: String, limit: Option<usize>),
    db_apply(connection: model::Connection, schema: String, table: String, edits: Vec<dbconn::CellEdit>),
    db_params(),
    repeat_preview(repeat: model::Repeat),
    sql_file_save(text: String, path: Option<String>, name: Option<String>, ext: Option<String>),
    activity_ping(seconds: f64, area: Option<String>),
    save_text_file(text: String, name: String, ext: String, what: String),
    sql_file_open(ext: Option<String>),
    db_demo_path(),
    list_fonts(),
    font_files(),
    field_values(dataset: String, field: String),
    python_info(),
    python_envs(),
    python_test(path: String),
    python_policy(on: Option<bool>),
    py_run(code: String, data: Json),
    export(format: String, pages: Json),
    reveal(path: String, open: bool),
    js_log(level: String, msg: String),
}

#[cfg(test)]
mod tests {
    use super::*;


    /// A stand-in provider on a local port: answers each request in turn and keeps what it was sent.
    fn fake_provider(answers: Vec<String>) -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for a in answers {
                let (mut sock, _) = listener.accept().unwrap();
                let mut buf = vec![0u8; 1 << 20];
                let mut got = Vec::new();
                // read headers and the body (Content-Length)
                loop {
                    let n = sock.read(&mut buf).unwrap();
                    got.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&got).to_string();
                    if let Some(i) = t.find("\r\n\r\n") {
                        let len: usize = t.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
                        if got.len() >= i + 4 + len { break; }
                    }
                    if n == 0 { break; }
                }
                seen2.lock().unwrap().push(String::from_utf8_lossy(&got).to_string());
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{a}", a.len());
                sock.write_all(resp.as_bytes()).unwrap();
            }
        });
        (port, seen)
    }

    fn fake_settings(port: u16) -> Settings {
        let mut settings = Settings::default();
        settings.ai.providers.insert("deepseek".into(), ai::ProviderSettings { model: "test-model".into(), endpoint: format!("http://127.0.0.1:{port}"), scope: String::new() });
        settings
    }

    /// The SQL assistant end to end against a stand-in provider: the first answer fails in the
    /// database, the error goes back, the second answer runs. Only structure is sent.
    #[test]
    fn sql_assistant_retries_with_the_database_error() {
        let (port, seen) = fake_provider(vec![
            r#"{"choices":[{"message":{"content":"{\"sql\": \"select nope from clients\", \"explanation\": \"x\"}"}}]}"#.into(),
            r#"{"choices":[{"message":{"content":"```json\n{\"sql\": \"select client_id, client from clients\", \"explanation\": \"Клиенты\"}\n```"}}]}"#.into(),
        ]);
        let settings = fake_settings(port);
        let dir = std::env::temp_dir().join(format!("quire_ai_test_{port}"));
        let tables = json!([{"schema": "main", "name": "clients", "kind": "table", "columns": [{"name": "client_id", "type": "varchar"}, {"name": "client", "type": "varchar"}], "rows": 41}]);
        let params = json!([{"name": "client", "value": "C0001", "about": "the report's client"}]);
        let mut runs = 0;
        let r = ai_sql_core("tester", &settings, (ai::provider("deepseek").unwrap(), "sk-test".into(), "personal"), &dir, "duckdb", &tables, &params, "ru", "write", "все клиенты", "", "", &[],
            |q| { runs += 1; if q.contains("nope") { Err("Binder Error: column nope not found".into()) } else { Ok(json!({"columns": [{"name": "client_id"}, {"name": "client"}], "total": 41})) } }).unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["attempts"], 2);
        assert_eq!(r["sql"], "select client_id, client from clients");
        assert_eq!(runs, 2);
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].contains("Authorization: Bearer sk-test") && reqs[0].contains("/chat/completions"), "{}", &reqs[0][..300.min(reqs[0].len())]);
        assert!(reqs[0].contains("clients(client_id varchar, client varchar)") && reqs[0].contains(":client"));
        assert!(!reqs[0].contains("C0001"), "parameter values never leave");
        assert!(reqs[0].contains("Write the explanation in Russian.") && reqs[0].contains("always Russian"), "the app's language goes with the request");
        assert!(reqs[1].contains("column nope not found"), "the database error goes back");
        // (the body only: the stand-in server's port is in the headers and may hold the digits)
        assert!(!reqs.iter().any(|q| q.split_once("\r\n\r\n").map_or(q.as_str(), |x| x.1).contains("41")), "row counts stay home");
        let log = ai::read_log(&dir, 5);
        assert_eq!(log[0]["feature"], "sql.write");
        assert_eq!(log[0]["ok"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The administrator's assistant: names, logins and client codes never leave; the answer's
    /// placeholders come back as names; an access request becomes exact changes; refs become links.
    #[test]
    fn admin_assistant_masks_people_and_resolves_access() {
        let mut store = auth::seed();
        // the bankers' group may send figures
        if let Some(g) = store.groups.iter_mut().find(|g| g.id == "bankers") { g.ai_level = "aggregates".into(); }
        let opts = auth::options(&store).expect("the demo bank database");
        let client = opts["clients"][0]["name"].as_str().unwrap().to_string();
        let client_id = opts["clients"][0]["id"].as_str().unwrap().to_string();
        let warren = store.users.iter().find(|u| u.login == "user").unwrap().name.clone();
        let (port, seen) = fake_provider(vec![
            r#"{"choices":[{"message":{"content":"{\"answer\": \"**[P-4]** signed in once\", \"refs\": [\"E1\"]}"}}]}"#.into(),
        ]);
        let settings = fake_settings(port);
        let dir = std::env::temp_dir().join(format!("quire_admin_ai_{port}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        audit::record(Some(&dir), "user", "auth", "sign-in", "user", "Signed in", Json::Null, None);
        // the context as the model would get it
        let (text, mut m) = ai_admin::access_ctx(&store, &opts);
        assert!(!text.contains(&warren) && !text.contains(&client) && !text.contains(&client_id), "names and client codes stay home");
        let tok_p = m.mask(&warren);
        let tok_c = m.mask(&client);
        assert!(tok_p.starts_with("[P-") && tok_c.starts_with("[CL-"), "{tok_p} {tok_c}");
        assert!(text.contains("user") && text.contains("admin"), "roles read as roles, not as logins");
        // AI levels by the names the app shows, with where each comes from
        assert!(text.contains("AI SETTINGS") && text.contains("Structure only") && text.contains("set for the group"), "{text}");
        assert!(!text.contains("AI aggregates") && !text.contains("AI structure") && !text.contains("(from "), "no internal level codes: {text}");
        if std::env::var("SHOW_ADMIN").is_ok() { eprintln!("{}", text.lines().take(12).collect::<Vec<_>>().join("\n")); }
        // the model's proposal, put back and made exact
        let mut answer = json!({"explanation": "x", "changes": [{"person": tok_p, "products": ["INS"], "clients": {"all": false, "bankers": [], "clients": [tok_c]}, "groups": ["bankers"], "ai_level": "structure"}], "questions": []});
        m.unmask_json(&mut answer);
        let plan = ai_admin::resolve_access(&store, &opts, &answer);
        let c = &plan[0];
        assert_eq!(c["problems"], json!([]), "{plan:#}");
        assert_eq!(c["after"]["login"], "user");
        assert_eq!(c["after"]["access"]["sources"], json!(["INS"]));
        assert_eq!(c["after"]["access"]["clients"], json!([client_id]), "the client's code, found again by name");
        assert_eq!(c["after"]["ai_level"], "structure");
        assert_eq!(c["groups"], json!(["bankers"]));
        // an unknown person is a problem, not a guess
        let bad = ai_admin::resolve_access(&store, &opts, &json!({"changes": [{"person": "Nobody Here"}]}));
        assert!(bad[0]["problems"][0].as_str().unwrap().contains("Nobody Here"));
        // a question: the answer's reference becomes a link to the event, its placeholder a name
        let r = ai_admin_run("admin", &settings, (ai::provider("deepseek").unwrap(), "sk-test".into(), "bank"), &dir, Some(&dir), &store, "ask", &format!("What did {warren} do?"), None, None, &[], "en");
        let reqs = seen.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&dir);
        let r = r.unwrap();
        assert!(!reqs[0].contains(&warren), "the question's name is masked too");
        assert!(r["answer"].as_str().unwrap().contains(&warren), "{r}");
        assert_eq!(r["links"][0]["type"], "event");
        assert_eq!(r["links"][0]["user"], "user");
    }

    /// A question about the database is answered in words (nothing runs); "make it a source"
    /// swaps literals for parameters, runs the result and says what changed.
    #[test]
    fn sql_assistant_answers_schema_questions_and_makes_sources() {
        let (port, seen) = fake_provider(vec![
            r#"{"choices":[{"message":{"content":"{\"explanation\": \"Курсы лежат в `fx_rates`.\"}"}}]}"#.into(),
            r#"{"choices":[{"message":{"content":"{\"sql\": \"select client_id, client from clients where client_id = :client\", \"explanation\": \"x\", \"name\": \"Клиент\", \"date_column\": \"\", \"changes\": [\"'C0001' → :client\"], \"left\": []}"}}]}"#.into(),
        ]);
        let settings = fake_settings(port);
        let dir = std::env::temp_dir().join(format!("quire_ai_test_{port}"));
        let tables = json!([{"schema": "main", "name": "clients", "kind": "table", "columns": [{"name": "client_id", "type": "varchar"}, {"name": "client", "type": "varchar"}], "rows": 41}]);
        let params = json!([{"name": "client", "value": "C0001", "about": "the report's client"}]);
        let pick = || (ai::provider("deepseek").unwrap(), "sk-test".to_string(), "personal");
        let mut runs = 0;
        let r = ai_sql_core("tester", &settings, pick(), &dir, "duckdb", &tables, &params, "ru", "write", "где курсы валют?", "", "", &[], |_| { runs += 1; Ok(json!({})) }).unwrap();
        assert_eq!(r["ok"], true);
        assert!(r["sql"].is_null() && r["explanation"].as_str().unwrap().contains("fx_rates"));
        assert_eq!(runs, 0, "a question runs nothing");
        let r = ai_sql_core("tester", &settings, pick(), &dir, "duckdb", &tables, &params, "ru", "source", "", "select client_id, client from clients where client_id = 'C0001'", "", &[],
            |q| { runs += 1; assert!(q.contains(":client")); Ok(json!({"columns": [{"name": "client_id"}, {"name": "client"}], "total": 1})) }).unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["name"], "Клиент");
        assert_eq!(r["changes"][0], "'C0001' → :client");
        assert_eq!(runs, 1);
        let reqs = seen.lock().unwrap();
        assert!(reqs[1].contains("Turn this query into a report source"));
        assert_eq!(ai::read_log(&dir, 5)[0]["feature"], "sql.source");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The design assistant end to end: a plan naming a missing field goes back with the problem,
    /// the corrected one becomes a new page in one undo step; only structure is sent.
    #[test]
    fn design_assistant_builds_a_page_in_one_undo_step() {
        let mut app = app_with(sample::template());
        let doc = app.doc.as_ref().unwrap();
        let ds = doc.datasets[0].clone();
        let num = ds.fields.iter().find(|f| f.kind == "number").unwrap().name.clone();
        let text = ds.fields.iter().find(|f| f.kind == "text").unwrap().name.clone();
        let g = vm::grid(doc);
        let first = doc.pages[0].id.clone();
        let pages = doc.pages.len();
        let w = |body: String| serde_json::to_string(&json!({"choices": [{"message": {"content": body}}]})).unwrap();
        let bad = json!({"explanation": "x", "title": "Overview", "widgets": [
            {"kind": "kpi", "x": 0, "y": 0, "w": 4, "h": 3, "title": "Total", "binding": {"dataset": ds.id, "measures": [{"field": "nope", "agg": "sum"}]}}]}).to_string();
        let good = json!({"explanation": "A KPI and a bar chart.", "title": "Overview", "widgets": [
            {"kind": "heading", "x": 0, "y": 0, "w": g.cols, "h": 1, "text": "Overview"},
            {"kind": "kpi", "x": 0, "y": 1, "w": 4, "h": 3, "title": "Total", "binding": {"dataset": ds.id, "measures": [{"field": num, "agg": "sum"}]}},
            {"kind": "bar", "x": 0, "y": 4, "w": g.cols, "h": 6, "title": "By category", "binding": {"dataset": ds.id, "dims": [text], "measures": [{"field": num, "agg": "sum"}], "sort": "value_desc"}}]}).to_string();
        let (port, seen) = fake_provider(vec![w(bad), w(good)]);
        let settings = fake_settings(port);
        let dir = std::env::temp_dir().join(format!("quire_ai_design_{port}"));
        let ctx = ai_design::context(app.doc.as_ref().unwrap(), None);
        let r = ai_design_core("tester", &settings, (ai::provider("deepseek").unwrap(), "sk-test".into(), "personal"), &dir, &ctx, "en", "page", "an overview page", &[],
            |ans, last| design_apply(&mut app, None, Some(&first), ans, last)).unwrap();
        assert_eq!(r["attempts"], 2, "{r}");
        assert_eq!(r["added"], 3);
        let doc = app.doc.as_ref().unwrap();
        assert_eq!(doc.pages.len(), pages + 1);
        let np = &doc.pages[1];
        assert_eq!(np.title, "Overview");
        assert_eq!(np.widgets.len(), 3);
        let reqs = seen.lock().unwrap();
        assert!(reqs[1].contains("no field") && reqs[1].contains("nope"), "the problem goes back");
        assert!(reqs[0].contains(&ds.id) && reqs[0].contains(&num));
        // one undo takes the whole page away
        let prev = app.undo.pop().unwrap();
        assert_eq!(prev.pages.len(), pages);
        assert_eq!(ai::read_log(&dir, 5)[0]["feature"], "design.page");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Changing the page in front: kept widgets by id (only what the answer gives changes),
    /// the rest removed — one undo step brings them all back.
    #[test]
    fn design_assistant_changes_the_page_in_front() {
        let mut app = app_with(sample::template());
        let doc = app.doc.as_ref().unwrap();
        let p = doc.pages.iter().find(|p| p.widgets.len() >= 3).unwrap().clone();
        let keep = &p.widgets[0];
        let ans = json!({"title": p.title, "widgets": [{"id": keep.id, "title": "Renamed", "x": 0, "y": 0}]});
        let r = design_apply(&mut app, Some(&p.id), None, &ans, true).unwrap();
        assert_eq!(r["changed"], 1);
        assert_eq!(r["removed"], p.widgets.len() - 1);
        let now = app.doc.as_ref().unwrap().pages.iter().find(|x| x.id == p.id).unwrap();
        assert_eq!(now.widgets.len(), 1);
        assert_eq!(now.widgets[0].id, keep.id);
        assert_eq!(now.widgets[0].title, "Renamed");
        assert_eq!(serde_json::to_string(&now.widgets[0].binding).unwrap(), serde_json::to_string(&keep.binding).unwrap(), "its data stays");
        let prev = app.undo.last().unwrap();
        assert_eq!(prev.pages.iter().find(|x| x.id == p.id).unwrap().widgets.len(), p.widgets.len());
        // an unknown id goes back to the model and changes nothing
        let bad = json!({"widgets": [{"id": "w999999", "title": "x"}]});
        let undo_len = app.undo.len();
        assert!(design_apply(&mut app, Some(&p.id), None, &bad, false).unwrap_err()[0].contains("no widget"));
        assert_eq!(app.undo.len(), undo_len);
    }

    /// Restyling after a picture: the picture goes with the request, a look with unreadable
    /// text goes back, the corrected one becomes the template's theme in one undo step.
    #[test]
    fn restyle_builds_the_theme_from_the_pictures_look() {
        let mut app = app_with(uhnwi_sample::template());
        let base = ai_restyle::base_look(&app.doc.as_ref().unwrap().theme);
        let w = |body: String| serde_json::to_string(&json!({"choices": [{"message": {"content": body}}]})).unwrap();
        let bad = json!({"explanation": "x", "name": "Blue", "look": {"colors": {"ink": "#f0f0f0", "page": "#ffffff"}},
            "views": {"bar/ranked": {"opts": {"orientation": "vertical"}}}}).to_string();
        let good = json!({"explanation": "Navy and white, square bars.", "name": "Морской", "look": {
            "colors": {"page": "#ffffff", "ink": "#0b1f3a", "accent": "#0b5cad"}, "type": {"display": "Inter", "ui": "Inter"},
            "shape": {"bar_radius": 0, "grid": "dashed", "cards": "plain"}},
            "views": {"kpi/hero": {"style": {"fonts": {"kpi": {"weight": 400}}}}, "table/lines": {"opts": {"table": {"totalLine": "single", "headLineWidth": 0.5}}}},
            "page": {"header": {"line": "bottom", "line_color": "$grid"}, "tab_style": {"style": "underline", "fill": "#0b5cad"}},
            "changes": ["KPI: light numbers", "Tables: hairlines"]}).to_string();
        let (port, seen) = fake_provider(vec![w(bad), w(good.clone()), w(good)]);
        let mut settings = Settings::default();
        settings.ai.providers.insert("openai".into(), ai::ProviderSettings { model: "test-model".into(), endpoint: format!("http://127.0.0.1:{port}"), scope: String::new() });
        let dir = std::env::temp_dir().join(format!("quire_ai_restyle_{port}"));
        let img = ai::Image { media: "image/png".into(), data: "iVBORw0KGgo=".into() };
        let undo = app.undo.len();
        let (styles, used) = (app.doc.as_ref().unwrap().theme.styles.clone(), views_used(app.doc.as_ref().unwrap()));
        let r = ai_restyle_core("tester", &settings, (ai::provider("openai").unwrap(), "sk-test".into(), "personal"), &dir, &base, &styles, &used, "ru-RU", "en", "", std::slice::from_ref(&img), &[],
            |l, v, p, last| restyle_apply(&mut app, l, v, p, last)).unwrap();
        // the second answer's typeface cuts off a heading: it goes back, and the last try is kept, its problems listed
        assert_eq!(r["attempts"], 3, "{r}");
        assert_eq!(r["ok"], false);
        assert!(r["issues"][0].as_str().unwrap().contains("cut off"), "{r}");
        let th = &app.doc.as_ref().unwrap().theme;
        assert_eq!(th.name, "Морской");
        assert_eq!(th.accent, "#0b5cad");
        assert_eq!(th.page_bg, "#ffffff");
        assert_eq!(th.locale, "ru-RU", "the template keeps its language");
        assert_eq!(th.look["colors"]["accent"], "#0b5cad", "the look is kept for the next restyle");
        assert_eq!(ai_restyle::base_look(th).accent, "#0b5cad");
        assert_eq!(app.undo.len(), undo + 1, "one undo step");
        let hero = th.styles["kpi"].iter().find(|v| v.id == "hero").unwrap();
        assert_eq!(hero.style["fonts"]["kpi"]["weight"], 400, "a view's change is applied");
        let lines = th.styles["table"].iter().find(|v| v.id == "lines").unwrap();
        assert_eq!(lines.opts["table"]["totalLine"], "single");
        assert_eq!(th.look["views"]["kpi/hero"]["style"]["fonts"]["kpi"]["weight"], 400, "kept for the next restyle");
        assert_eq!(app.doc.as_ref().unwrap().page.header.line, "bottom");
        assert_eq!(th.tabs["style"], "underline", "the section tabs follow the restyle");
        assert_eq!(r["changes"][0], "KPI: light numbers");
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].contains("image_url") && reqs[0].contains("data:image/png;base64,iVBORw0KGgo="), "the picture goes with the request");
        assert!(reqs[1].contains("contrast") && reqs[1].contains("opts.orientation can't be changed"), "the problems go back");
        assert!(reqs[0].contains("kpi/hero") && reqs[0].contains("[used by"), "the views go with the request");
        assert!(reqs[2].contains("cut off") && reqs[2].contains("[view heading/"), "what breaks goes back, naming its view");
        // a provider that can't read pictures says so before sending anything
        let tmp = ai::tmp_dir(&dir);
        let c = ai::Client::new(ai::provider("deepseek").unwrap(), &ai::ProviderSettings::default(), "k".into(), &tmp);
        assert!(c.chat_with("s", &[ai::Msg::user("x")], Some((std::slice::from_ref(&img), 0)), 10).unwrap_err().contains("can't read pictures"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A heading's badge: a filled chip, a frame, or the text alone.
    #[test]
    fn heading_badges_take_their_style() {
        let doc = uhnwi_sample::template();
        let (pi, wi) = doc.pages.iter().enumerate().find_map(|(pi, p)| p.widgets.iter().position(|w| w.kind == "heading" && w.opt_str("badge").is_some_and(|b| !b.is_empty())).map(|wi| (pi, wi))).expect("a heading with a badge");
        let rects = |style: Option<&str>| -> usize {
            let mut d = doc.clone();
            let w = &mut d.pages[pi].widgets[wi];
            if let Some(s) = style { w.opts["badgeStyle"] = json!(s); }
            let mut app = app_with(d);
            let st = app.resp();
            let id = app.doc.as_ref().unwrap().pages[pi].widgets[wi].id.clone();
            let wv = st["pages"][pi]["widgets"].as_array().unwrap().iter().find(|x| x["id"] == id.as_str()).unwrap().clone();
            wv["deco"].as_array().map(|a| a.iter().filter(|x| x["kind"] == "rect").count()).unwrap_or(0)
        };
        let solid = rects(None);
        assert_eq!(rects(Some("outline")), solid + 1, "a frame is two rects");
        assert_eq!(rects(Some("text")), solid - 1, "text alone has no chip");
    }

    /// A table that continues keeps the row height it is given: taller rows carry on to a new
    /// page instead of being squeezed back (even with "fill height", which only stretches).
    #[test]
    fn taller_rows_continue_instead_of_squeezing() {
        let mut app = app_with(uhnwi_sample::template());
        let doc = app.doc.as_ref().unwrap();
        // the bank accounts' structure: one grouped table that fills its place
        let (pi, id) = doc.pages.iter().enumerate().find(|(_, p)| p.title.contains("Банковские счета · структура"))
            .and_then(|(pi, p)| p.widgets.iter().find(|w| w.kind == "table").map(|w| (pi, w.id.clone()))).unwrap();
        let pages = doc.pages.len();
        let rows_on = |app: &mut App| -> (u64, u64, f64) {
            let st = app.resp();
            let wv = st["pages"][pi]["widgets"].as_array().unwrap().iter().find(|w| w["id"] == id.as_str()).unwrap().clone();
            let t = &wv["table"];
            (t["visible"].as_u64().unwrap(), t["rows"].as_u64().unwrap(), t["heights"].as_array().unwrap().iter().filter_map(|h| h.as_f64()).fold(f64::MAX, f64::min))
        };
        let (vis, rows, _) = rows_on(&mut app);
        assert_eq!(vis, rows, "as designed, every row fits");
        run_edit(&mut app, serde_json::from_value(json!({"op": "update_widget", "id": id, "patch": {"opts": {"table": {"row_h": 24}}}})).unwrap()).unwrap();
        let (vis, rows, min_h) = rows_on(&mut app);
        assert!(vis < rows, "taller rows don't all fit: {vis} of {rows}");
        assert!(min_h >= 24.0 - 1e-6, "and keep their height ({min_h})");
        assert_eq!(app.doc.as_ref().unwrap().pages.len(), pages + 1, "the rest continues on a page added after it");
    }

    /// Dragging a table taller pushes the widgets linked to its end past the page: its last row goes
    /// on to the next page and they go after it there; dragging it back brings them all back.
    #[test]
    fn a_taller_table_takes_its_linked_widgets_to_the_next_page() {
        let mut app = app_with(uhnwi_sample::template());
        let doc = app.doc.as_ref().unwrap();
        let (pi, tbl) = doc.pages.iter().enumerate().find(|(_, p)| p.title.contains("Банковские счета · структура"))
            .and_then(|(pi, p)| p.widgets.iter().find(|w| w.kind == "table").map(|w| (pi, w.clone()))).unwrap();
        let linked: Vec<(String, u32)> = doc.pages[pi].widgets.iter().filter(|w| vm::follows(w) == Some(tbl.id.as_str())).map(|w| (w.id.clone(), w.y)).collect();
        assert!(!linked.is_empty());
        let pages = doc.pages.len();
        let state = |app: &mut App| {
            let st = app.resp();
            let p = &st["pages"][pi];
            let t = p["widgets"].as_array().unwrap().iter().find(|w| w["id"] == tbl.id.as_str()).unwrap()["table"].clone();
            let problems = p["issues"].as_array().unwrap().iter().filter(|i| i["severity"] != "info").count();
            (t["visible"].as_u64().unwrap(), t["rows"].as_u64().unwrap(), problems)
        };
        let set = |app: &mut App, h: u32| run_edit(app, serde_json::from_value(json!({"op": "set_rect", "id": tbl.id, "rect": {"x": tbl.x, "y": tbl.y, "w": tbl.w, "h": h}})).unwrap()).unwrap();
        set(&mut app, tbl.h + 1);
        let (vis, rows, problems) = state(&mut app);
        assert_eq!(vis + 1, rows, "the last row goes on");
        assert_eq!(problems, 0, "nothing is left running past the page");
        assert_eq!(app.doc.as_ref().unwrap().pages.len(), pages + 1);
        assert!(linked.iter().all(|(id, _)| app.doc.as_ref().unwrap().moved.contains(id)), "the linked widgets go with it");
        set(&mut app, tbl.h);
        let (vis, rows, problems) = state(&mut app);
        assert_eq!((vis, problems), (rows, 0));
        assert_eq!(app.doc.as_ref().unwrap().pages.len(), pages);
        for (id, y) in &linked {
            assert_eq!(app.doc.as_ref().unwrap().pages[pi].widgets.iter().find(|w| &w.id == id).unwrap().y, *y, "back where they were");
        }
    }

    /// What the report assistant sends: the report's figures page by page, the client and every
    /// account or contract code masked — and nothing when the bank allows only structure.
    /// A report made for a client is that client's from the start: no detour through the template's default client.
    #[test]
    fn a_new_report_starts_with_its_client() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(sample::blank());
        app.load(for_client(d, Some("James Hardenberg")), None);
        let st = app.resp();
        assert_eq!(app.doc.as_ref().unwrap().params.values.get("client").map(String::as_str), Some("James Hardenberg"), "{}", st["doc"]["params"]);
    }

    /// A batch report is made apart from the open document (which stays as it is), for the
    /// client and period asked; a client outside the person's access is refused; files are written.
    #[test]
    fn batch_reports_leave_the_open_document_alone() {
        if dbconn::demo_path().is_none() { return }
        let mut doc = uhnwi_fixed();
        to_report(&mut doc, None);
        let mut app = app_with(doc);
        let s = auth::sign_in(&auth::seed(), "user", auth::DEMO_PASSWORD).unwrap();
        app.start_session(s).unwrap();
        let before = serde_json::to_string(app.doc.as_ref().unwrap()).unwrap();
        let d = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
        let (_, st) = batch_make(&app, "pub:uhnwi", "James Hardenberg", d("2026-07-01"), d("2026-09-30")).unwrap();
        assert_eq!(st["doc"]["params"]["values"]["client"], "James Hardenberg");
        assert_eq!(st["doc"]["params"]["start"], "2026-07-01");
        assert_eq!(st["doc"]["params"]["date"], "2026-09-30");
        assert!(st["pages"].as_array().unwrap().len() > 10);
        assert_eq!(serde_json::to_string(app.doc.as_ref().unwrap()).unwrap(), before, "the open document is untouched");
        assert!(app.parked.is_empty(), "no document was opened");
        // not this banker's client
        assert!(batch_make(&app, "pub:uhnwi", "Stephen Curryman", None, None).is_err());
        // a file in the folder
        let dir = std::env::temp_dir().join(format!("quire_batch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="842" height="595"><rect width="10" height="10"/></svg>"#;
        let path = batch_write(&app, dir.to_str().unwrap(), "James Hardenberg — Отчёт: Q3", "pdf", json!([svg]), 842.0, 595.0, "James Hardenberg", "Отчёт").unwrap();
        assert!(path.ends_with("James Hardenberg — Отчёт- Q3.pdf") && std::fs::metadata(&path).unwrap().len() > 100, "{path}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A long proposal (as the AI wrote one) never cuts its text: it goes onto two pages, and a
    /// masked account in bold ("**Концентрация на ***23990**") keeps its bold.
    /// The register keeps every proposal: made (with the report, client and who), changed (a version),
    /// removed from the report (still kept); a banker sets its status only for their own clients; the
    /// report's AI reads the client's earlier proposals.
    #[test]
    fn proposals_are_kept_in_the_register() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        let dir = std::env::temp_dir().join(format!("quire_reg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        app.store_path = Some(dir.join("store.json"));
        let s = auth::sign_in(&auth::seed(), "user", auth::DEMO_PASSWORD).unwrap();
        app.start_session(s).unwrap();
        let r = proposal_apply_as(&mut app, None, proposal::plain("План\n\n- Купить ОФЗ на 5 млн ₽", true), "План\n\n- Купить ОФЗ на 5 млн ₽", "notes").unwrap();
        let (id, uid) = (r["id"].as_str().unwrap().to_string(), r["uid"].as_str().unwrap().to_string());
        assert_eq!(proposal::uid_of(app.doc.as_ref().unwrap(), &id).as_deref(), Some(uid.as_str()), "the page carries its register id");
        let e = proposals::get(Some(&dir), &uid).unwrap();
        assert_eq!((e["status"].as_str(), e["created_by"].as_str(), e["source"].as_str()), (Some("draft"), Some("user"), Some("notes")));
        let client = e["place"]["client"].as_str().unwrap().to_string();
        assert!(!client.is_empty() && !e["place"]["client_id"].as_str().unwrap().is_empty(), "{e}");
        // changed in the report: the same entry, its old content a version
        proposal_apply_as(&mut app, Some(id.clone()), proposal::plain("План Б\n\n- Продать фонд", true), "План Б", "ai").unwrap();
        let e = proposals::get(Some(&dir), &uid).unwrap();
        assert_eq!((e["content"]["title"].as_str(), e["versions"].as_array().unwrap().len()), (Some("План Б"), 1));
        // the banker's own list; the report's AI sees it
        assert_eq!(proposals_list_run(&app, None, false).unwrap().as_array().unwrap().len(), 1);
        assert!(proposals_list_run(&app, None, true).is_err(), "everyone's: administrators only");
        let (facts, _, _) = report_facts(&mut app, None).unwrap_or_default();
        assert!(facts.is_empty() || facts.contains("EARLIER PROPOSALS"), "the register goes to the AI when figures may");
        // status: yes for their client; another banker's client, no
        assert_eq!(proposal_status_run(&app, &uid, "presented", "").unwrap()["status"], "presented");
        let mut other = proposals::get(Some(&dir), &uid).unwrap();
        other["place"]["client"] = json!("Stephen Curryman");
        let mut all: Vec<Json> = serde_json::from_str(&std::fs::read_to_string(dir.join("proposals.json")).unwrap()).unwrap();
        let mut copy = other.clone();
        copy["uid"] = json!("Pother");
        all.push(copy);
        std::fs::write(dir.join("proposals.json"), serde_json::to_string(&all).unwrap()).unwrap();
        assert!(proposal_status_run(&app, "Pother", "accepted", "").is_err(), "not this banker's client");
        assert_eq!(proposals_list_run(&app, None, false).unwrap().as_array().unwrap().len(), 1, "only their clients");
        // removed from the report: kept in the register
        proposal_remove_run(&mut app, &id).unwrap();
        assert_eq!(proposals::get(Some(&dir), &uid).unwrap()["in_report"], false);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Exporting the report (PDF / PowerPoint) presents its proposals: a draft becomes "presented",
    /// one already accepted keeps its status, and one made before the register gets its entry.
    #[test]
    fn exporting_presents_the_proposals() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        let dir = std::env::temp_dir().join(format!("quire_exp_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        app.store_path = Some(dir.join("store.json"));
        app.start_session(auth::sign_in(&auth::seed(), "user", auth::DEMO_PASSWORD).unwrap()).unwrap();
        let a = proposal_apply(&mut app, None, proposal::plain("План А\n\nКоротко.\n\n- Купить ОФЗ на 5 млн ₽\n\nРиски:\n- Ставки", true), "А").unwrap();
        let b = proposal_apply(&mut app, None, proposal::plain("План Б\n\n- Продать фонд", true), "Б").unwrap();
        let c = proposal_apply(&mut app, None, proposal::plain("План В\n\n- Открыть вклад", true), "В").unwrap();
        let (ua, ub) = (a["uid"].as_str().unwrap().to_string(), b["uid"].as_str().unwrap().to_string());
        proposal_status_run(&app, &ub, "accepted", "").unwrap();
        // В as made before the register: no entry, no id on its page
        let cid = c["id"].as_str().unwrap().to_string();
        let uc = c["uid"].as_str().unwrap().to_string();
        let mut all: Vec<Json> = serde_json::from_str(&std::fs::read_to_string(dir.join("proposals.json")).unwrap()).unwrap();
        all.retain(|r| r["uid"] != uc.as_str());
        std::fs::write(dir.join("proposals.json"), serde_json::to_string(&all).unwrap()).unwrap();
        let g = proposal::group(&cid);
        for w in app.doc.as_mut().unwrap().pages.iter_mut().filter(|p| p.group == g).flat_map(|p| p.widgets.iter_mut()) {
            if let Some(o) = w.opts.as_object_mut() { o.remove("proposalUid"); }
        }
        proposals_went_out(&mut app, "pdf");
        let st = |u: &str| proposals::get(Some(&dir), u).unwrap()["status"].as_str().unwrap().to_string();
        assert_eq!(st(&ua), "presented", "a draft is presented");
        assert_eq!(st(&ub), "accepted", "an outcome stays");
        let new = proposal::uid_of(app.doc.as_ref().unwrap(), &cid).expect("the older one got its entry");
        let e = proposals::get(Some(&dir), &new).unwrap();
        assert_eq!((e["status"].as_str(), e["content"]["title"].as_str()), (Some("presented"), Some("План В")));
        assert_eq!(e["content"]["actions"][0]["action"], "Открыть", "read from its pages: {e}");
        assert!(e["history"].as_array().unwrap().iter().any(|h| h["note"] == "Report exported to PDF"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Many actions: the table goes on over a page of its own, then the cards; nothing hidden.
    #[test]
    fn many_actions_go_on_over_further_pages() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        let acts: Vec<Json> = (1..=8).map(|i| json!({"action": "Купить", "what": format!("Облигации выпуска {i} с длинным названием эмитента"), "amount": "до 10 млн ₽", "why": "Фиксированный доход на срок до погашения"})).collect();
        let lead = "Предлагаем ".to_string() + &"пересмотреть структуру портфеля и разместить возвраты по вкладам; ".repeat(6);
        let spec = json!({"title": "Много действий", "lead": lead, "actions": acts, "effect": ["**Доход** — регулярный", "**Риск** — ниже", "**Срок** — до 3 лет"],
            "risks": ["**Ставки** могут снизиться"], "steps": ["**Встреча** — обсудить"]});
        let r = proposal_apply(&mut app, None, spec, "").unwrap();
        let doc = app.doc.as_ref().unwrap();
        let g = proposal::group(r["id"].as_str().unwrap());
        let pages: Vec<usize> = doc.pages.iter().enumerate().filter(|(_, p)| p.group == g).map(|(i, _)| i).collect();
        assert!(pages.len() >= 3, "the actions go on over a page: {:?}", pages.iter().map(|&i| &doc.pages[i].title).collect::<Vec<_>>());
        assert!(pages.windows(2).all(|w| w[1] == w[0] + 1), "together");
        assert!(doc.pages[pages[1]].auto && doc.pages[pages[1]].toc_hidden, "the table's own continuation page, out of the contents");
        assert!(doc.pages[*pages.last().unwrap()].widgets.iter().any(|w| w.title == "Риски"), "the cards after the actions");
        for &i in &pages {
            let bad: Vec<&Json> = r["st"]["pages"][i]["issues"].as_array().unwrap().iter().filter(|x| x["severity"] != "info").collect();
            assert!(bad.is_empty(), "page {}: {bad:?}", doc.pages[i].title);
        }
        // removed: its continuation page goes with it
        let id = r["id"].as_str().map(String::from);
        run_edit(&mut app, Edit::SetProposal { id, spec: None, notes: String::new(), split: false }).unwrap();
        assert!(!app.doc.as_ref().unwrap().pages.iter().any(|p| p.group == g || (p.auto && p.title.contains("Много действий"))));
    }

    /// A proposal made when a report held only one ("PROPOSAL") takes an id when the report opens.
    #[test]
    fn an_older_proposal_takes_an_id() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        proposal_apply(&mut app, None, proposal::plain("План\n\n- Купить фонд на 1 млн ₽", true), "План").unwrap();
        // as it was saved before
        let mut old = app.doc.clone().unwrap();
        for p in old.pages.iter_mut().filter(|p| proposal::is_proposal(p)) {
            p.group = proposal::GROUP.into();
            for w in &mut p.widgets { if let Some(b) = w.binding.as_mut() { if b.dataset.starts_with(proposal::DATASET) { b.dataset = proposal::DATASET.into(); } } }
        }
        for ds in old.datasets.iter_mut().filter(|d| d.id.starts_with(proposal::DATASET)) { ds.id = proposal::DATASET.into(); }
        app.load(old, None);
        let doc = app.doc.as_ref().unwrap();
        let list = proposal::list(doc);
        assert_eq!(list.len(), 1);
        assert!(doc.dataset(&proposal::dataset(&list[0].0)).is_some());
        let st = app.resp();
        let table = st["pages"][list[0].1]["widgets"].as_array().unwrap().iter().find(|w| w["data"]["rows"].is_array()).expect("the table still has its data");
        assert_eq!(table["data"]["rows"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_long_proposal_takes_two_pages_and_nothing_is_cut() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        let long = "За период портфель вырос на **10,7% — ниже медианы похожих клиентов (13,3%) и ставки по вкладу (12,9%)** (стр. 6). Основная причина — **высокая доля денежных средств и концентрация на одном накопительном счёте**; в ближайшие 12 месяцев возвращается 51,7 млн ₽ с закрывающихся вкладов (стр. 20) — удобный момент пересмотреть структуру.";
        let act = |a: &str, w: &str, m: &str, y: &str| json!({"action": a, "what": w, "amount": m, "why": y});
        let spec = json!({"title": "Пересмотреть структуру портфеля", "lead": long,
            "actions": [act("Распределить", "возврат капитала с закрывающихся вкладов «Сберегательный», «Капитал», «Стабильный»", "51,7 млн ₽ (13,5 млн ₽ 01.12.2026, 22,1 млн ₽ 20.01.2027, 16,2 млн ₽ 10.09.2027)", "Средства освобождаются и требуют решения о размещении"),
                act("Сократить", "долю накопительного счёта ***23990", "часть из 166,3 млн ₽ (53,1% портфеля)", "Одна позиция формирует более половины портфеля"),
                act("Перевести", "часть средств в облигации федерального займа", "часть из 51,7 млн ₽", "Фиксированный доход на срок"),
                act("Продлить", "вклад «Капитал» ***65988", "22,1 млн ₽", "Сохранить доход по ставке"),
                act("Вложить", "фонд денежного рынка", "часть свободных средств", "Ликвидность и доход")],
            "effect": ["**Снижение концентрации** — доля крупнейшей позиции может уйти ниже 53,1%.", "**Загрузка возвратов** — 51,7 млн ₽ получают назначение до поступления.", "**Регулярный доход** — 30,9 млн ₽ в год перестают простаивать на счетах."],
            "reasons": ["**Профиль консервативный** при доле денежных средств около 75% (стр. 4).", "**Концентрация на ***23990** — 53,1% портфеля и 69,4% банковского блока (стр. 4, 19).", "**Возвраты по вкладам** — 51,7 млн ₽ в ближайшие 12 месяцев (стр. 20)."],
            "risks": ["**Ставки по вкладам** могут измениться к датам продления (стр. 19).", "**Рыночная переоценка** инвестиционных инструментов — у ИИС просадки до −1,5% в месяц (стр. 11).", "**Налоги и комиссии** при продажах — за период 5,02 млн ₽ НДФЛ (стр. 24)."],
            "steps": ["**Сверить профиль риска** и целевую долю денежных средств с Вами (стр. 4).", "**Подготовить варианты** размещения 13,5 млн ₽ к 01.12.2026 — продление вклада или альтернативы (стр. 20).", "**Обсудить концентрацию** накопительного счёта на встрече."]});
        let r = proposal_apply(&mut app, None, spec, "").unwrap();
        let doc = app.doc.as_ref().unwrap();
        let pages: Vec<usize> = doc.pages.iter().enumerate().filter(|(_, p)| proposal::is_proposal(p)).map(|(i, _)| i).collect();
        assert_eq!(pages.len(), 2, "two pages");
        assert!(doc.pages[pages[1] + 1].title.contains("Юридическ"), "still before the legal text");
        for &i in &pages {
            let bad: Vec<&Json> = r["st"]["pages"][i]["issues"].as_array().unwrap().iter().filter(|x| x["severity"] != "info").collect();
            assert!(bad.is_empty(), "page {}: {bad:?}", doc.pages[i].title);
        }
        // the masked account no longer breaks the bold
        let texts: String = doc.pages[pages[1]].widgets.iter().filter_map(|w| w.opt_str("text")).collect();
        assert!(texts.contains("**Концентрация на ∗∗∗23990**"), "{texts}");
        // a short one stays on one page
        let id = r["id"].as_str().map(String::from);
        proposal_apply(&mut app, id, proposal::plain("План\n\nКоротко.\n\n- Купить фонд на 1 млн ₽\n\nРиски:\n- Рынок", true), "").unwrap();
        assert_eq!(app.doc.as_ref().unwrap().pages.iter().filter(|p| proposal::is_proposal(p)).count(), 1);
    }

    /// In a report (auto-arrange on), a widget made smaller and moved down a row stays there —
    /// also after later changes on the page — instead of being pulled back up.
    #[test]
    fn a_widget_placed_by_hand_stays_put() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        assert!(app.doc.as_ref().unwrap().arrange.auto && !app.doc.as_ref().unwrap().arrange.fill, "reports tidy up but don't stretch");
        let notes = "План\n\nКоротко.\n\n- Купить фонд на 1 млн ₽\n\nРиски:\n- Рынок\nСледующие шаги:\n- Встреча";
        proposal_apply(&mut app, None, proposal::plain(notes, true), notes).unwrap();
        let doc = app.doc.as_ref().unwrap();
        let p = doc.pages.iter().find(|p| proposal::is_proposal(p)).unwrap();
        let risks = p.widgets.iter().find(|w| w.title == "Риски").unwrap().clone();
        let steps = p.widgets.iter().find(|w| w.title == "Следующие шаги").unwrap().clone();
        let rect = Rect { x: risks.x, y: risks.y + 1, w: risks.w, h: risks.h - 1 };
        run_edit(&mut app, Edit::SetRect { id: risks.id.clone(), rect: rect.clone() }).unwrap();
        let now = |app: &App| { let (pi, wi) = app.doc.as_ref().unwrap().find_widget(&risks.id).unwrap(); let w = &app.doc.as_ref().unwrap().pages[pi].widgets[wi]; (w.x, w.y, w.w, w.h) };
        assert_eq!(now(&app), (rect.x, rect.y, rect.w, rect.h), "where it was put");
        // another change on the page: it still stays
        run_edit(&mut app, Edit::SetRect { id: steps.id.clone(), rect: Rect { x: steps.x, y: steps.y, w: steps.w, h: steps.h + 1 } }).unwrap();
        assert_eq!(now(&app), (rect.x, rect.y, rect.w, rect.h), "still where it was put");
    }

    /// The banker's proposal: a page before the legal text, its actions a table, read by the brief and
    /// the check as the banker's; made again it replaces itself; removed, it goes with its data.
    #[test]
    fn the_proposal_is_a_page_before_the_legal_text() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        let notes = "Снижение доли акций\n\nПредлагаем вернуть портфель к консервативному профилю.\n\n- Продать Банк ВТБ на 2 млн ₽ — доля акций выше профиля\n- Купить ОФЗ 26238 на 5 млн ₽\n\nРиски:\n- Ставка может снизиться";
        let r = proposal_apply(&mut app, None, proposal::plain(notes, true), notes).unwrap();
        let doc = app.doc.as_ref().unwrap();
        let pi = doc.pages.iter().position(|p| proposal::is_proposal(p)).unwrap();
        assert_eq!(r["page"], doc.pages[pi].id.as_str());
        assert!(doc.pages[pi + 1].title.contains("Юридическ"), "right before the legal text: {}", doc.pages[pi + 1].title);
        assert!(doc.pages[pi - 1].title.starts_with("Движение средств"), "after the money page: {}", doc.pages[pi - 1].title);
        // the actions table shows its rows; the page fits
        let pv = &r["st"]["pages"][pi];
        let table = pv["widgets"].as_array().unwrap().iter().find(|w| w["data"]["rows"].is_array()).expect("the actions table");
        assert_eq!(table["data"]["rows"].as_array().unwrap().len(), 2, "{}", table["data"]);
        assert!(pv["issues"].as_array().unwrap().iter().all(|i| i["severity"] == "info"), "{}", pv["issues"]);
        // the brief and the check read it, marked as the banker's
        let st = app.resp();
        let mut m = ai_report::Masker::new(&["Кирилл Шарихин".into()]);
        let facts = ai_report::facts(app.doc.as_ref().unwrap(), &st, None, &mut m);
        assert!(facts.contains("THE BANKER'S PROPOSAL") && facts.contains("Продать") && facts.contains("Ставка может снизиться"), "{}", &facts[facts.find("PROPOSAL").unwrap_or(0)..]);
        // a second proposal: after the first, before the legal text, with its own data
        let first = r["id"].as_str().unwrap().to_string();
        let r2 = proposal_apply(&mut app, None, proposal::plain("Другое\n\n- Купить фонд", true), "Другое").unwrap();
        let second = r2["id"].as_str().unwrap().to_string();
        assert_ne!(first, second);
        let doc = app.doc.as_ref().unwrap();
        assert_eq!(proposal::list(doc).iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(), [first.clone(), second.clone()], "in the order made");
        assert!(doc.dataset(&proposal::dataset(&first)).is_some() && doc.dataset(&proposal::dataset(&second)).is_some());
        let last = doc.pages.iter().rposition(|p| proposal::is_proposal(p)).unwrap();
        assert!(doc.pages[last + 1].title.contains("Юридическ"));
        // the first one changed in place: still first, the second untouched
        proposal_apply(&mut app, Some(first.clone()), proposal::plain("План Б\n\n- Продать фонд", true), "План Б").unwrap();
        let doc = app.doc.as_ref().unwrap();
        assert_eq!(proposal::list(doc).iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(), [first.clone(), second.clone()]);
        assert!(doc.pages.iter().any(|p| p.title.contains("План Б")) && doc.pages.iter().any(|p| p.title.contains("Другое")));
        // every view it uses is the theme's: the other themes stay on the report's theme list
        let fits: Vec<bool> = theme_list(None, app.doc.as_ref())["themes"].as_array().or(theme_list(None, app.doc.as_ref()).as_array()).cloned().unwrap_or_default().iter().filter_map(|t| t["fits"].as_bool()).collect();
        assert!(fits.iter().filter(|f| **f).count() >= 2, "other themes still fit: {fits:?}");
        // one removed: the other stays
        run_edit(&mut app, Edit::SetProposal { id: Some(first.clone()), spec: None, notes: String::new(), split: false }).unwrap();
        let doc = app.doc.as_ref().unwrap();
        assert_eq!(proposal::list(doc).len(), 1);
        assert!(doc.dataset(&proposal::dataset(&first)).is_none() && doc.dataset(&proposal::dataset(&second)).is_some());
        // all removed
        run_edit(&mut app, Edit::SetProposal { id: None, spec: None, notes: String::new(), split: false }).unwrap();
        let doc = app.doc.as_ref().unwrap();
        assert!(!doc.pages.iter().any(|p| proposal::is_proposal(p)) && !doc.datasets.iter().any(|d| d.id.starts_with(proposal::DATASET)));
        // a report user may add it (the template's page rules don't stop it)
        assert!(check_policy(doc, &Edit::SetProposal { id: None, spec: Some(json!({})), notes: String::new(), split: false }).is_ok());
    }

    /// A table's automatic continuation page shows the same account as the page it continues — also
    /// after the report switches to another client (it once kept the previous client's account).
    #[test]
    fn continuation_pages_follow_their_account_when_the_client_changes() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        for client in ["Кирилл Шарихин", "James Hardenberg"] {
            let mut p = app.doc.as_ref().unwrap().params.clone();
            p.values.insert("client".into(), client.into());
            run_edit(&mut app, Edit::SetParams { params: p }).unwrap();
            let doc = app.doc.as_ref().unwrap();
            for page in doc.pages.iter().filter(|p| p.auto) {
                let src = page.widgets.iter().find_map(|w| vm::continues(w)).expect("an automatic page continues a table");
                let from = doc.pages.iter().find(|p| p.widgets.iter().any(|w| w.id == src)).unwrap();
                assert_eq!(page.row, from.row, "{client}: page «{}» shows its own account", page.title);
                assert!(page.title.ends_with(" (продолжение)"), "in the report's language: {}", page.title);
            }
        }
    }

    /// An administrator's work is a session of its own: changes to people and the AI policy, and the time in Admin.
    #[test]
    fn admin_work_is_a_session() {
        let mut app = app_with(uhnwi_sample::template());
        let dir = std::env::temp_dir().join(format!("quire_admwork_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        app.store_path = Some(dir.join("store.json"));
        app.session = Some(auth::Session { login: "admin".into(), name: "Admin".into(), roles: auth::Roles { admin: true, ..Default::default() }, access: Default::default() });
        app.audit("admin", "change user", "user", "Changed the account user", json!({"before": {"name": "A"}, "after": {"name": "B"}}), None);
        app.audit("ai", "bank policy", "", "Data that may be sent: aggregates", Json::Null, None);
        app.work.borrow_mut().open.values_mut().for_each(|w| w.last -= std::time::Duration::from_secs(30));
        app.work_time(30.0, "admin");
        app.end_work_of(None, "signed out");
        let all = audit::sessions(Some(&dir), None, &audit::Filter::default());
        let _ = std::fs::remove_dir_all(&dir);
        let ss = all["sessions"].as_array().unwrap();
        assert_eq!(ss.len(), 1, "{all:#}");
        assert_eq!(ss[0]["kind"], "admin");
        assert_eq!(ss[0]["edits"], 2, "the account change and the policy");
        assert_eq!(ss[0]["active_s"], 30.0);
        assert_eq!(ss[0]["reason"], "signed out");
    }

    #[test]
    fn work_sessions_follow_the_document_and_its_client() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        let dir = std::env::temp_dir().join(format!("quire_work_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        app.store_path = Some(dir.join("store.json"));
        app.session = Some(auth::Session { login: "user".into(), name: "User".into(), roles: Default::default(), access: Default::default() });
        let with_client = |app: &App, c: &str| { let mut p = app.doc.as_ref().unwrap().params.clone(); p.values.insert("client".into(), c.to_string()); p };
        // opened, then a client picked straight away: still one session, now for that client
        app.audit("document", "new", "", "New document", Json::Null, None);
        let p = with_client(&app, "Кирилл Шарихин");
        run_edit(&mut app, Edit::SetParams { params: p }).unwrap();
        // work: an edit with its old and new value, time
        let wid = app.doc.as_ref().unwrap().pages[2].widgets.iter().find(|w| w.kind == "table").unwrap().id.clone();
        run_edit(&mut app, Edit::UpdateWidget { id: wid, patch: json!({"title": "Новый заголовок"}) }).unwrap();
        // (as if half a minute passed before each ping: a ping counts no more than the time since the last)
        for _ in 0..2 {
            app.work.borrow_mut().open.values_mut().for_each(|w| w.last -= std::time::Duration::from_secs(30));
            app.work_time(30.0, "");
        }
        // another client: a new session; an export and an AI request in it; then closed
        let p = with_client(&app, "Anthony Edwardson");
        run_edit(&mut app, Edit::SetParams { params: p }).unwrap();
        app.audit("document", "export", "", "Exported to PDF", Json::Null, None);
        let sid = app.work_id(true);
        ai::in_session(&sid, || ai::log(&dir.join("ai"), json!({"at": chrono::Local::now().to_rfc3339(), "user": "user", "feature": "report.brief", "ok": true, "tokens_in": 100, "tokens_out": 20, "session": sid})));
        let id = app.active_id;
        app.end_work_of(Some(id), "closed");
        let all = audit::sessions(Some(&dir), Some(&dir.join("ai")), &audit::Filter::default());
        let _ = std::fs::remove_dir_all(&dir);
        let ss = all["sessions"].as_array().unwrap();
        assert_eq!(ss.len(), 2, "{all:#}");
        let first = ss.iter().find(|x| x["client"] == "Кирилл Шарихин").expect("the first client's session");
        assert_eq!(first["reason"], "another client");
        assert_eq!(first["active_s"], 60.0);
        assert!(first["edits"].as_u64().unwrap() >= 2);
        let ch = first["events"].as_array().unwrap().iter().find_map(|e| e["data"]["changes"].as_array().filter(|c| c.iter().any(|x| x["path"] == "title")).cloned()).expect("the title change");
        assert_eq!(ch.iter().find(|x| x["path"] == "title").unwrap()["to"], "Новый заголовок");
        let second = ss.iter().find(|x| x["client"] == "Anthony Edwardson").expect("the second client's session");
        assert_eq!(second["reason"], "closed");
        assert_eq!(second["exports"].as_array().unwrap().len(), 1);
        assert_eq!((second["ai"].as_u64(), second["tokens_in"].as_u64()), (Some(1), Some(100)));
        assert!(second["period"].as_str().is_some_and(|p| !p.is_empty()));
    }

    #[test]
    fn report_facts_are_figures_without_identities() {
        let mut d = uhnwi_sample::template();
        to_report(&mut d, None);
        let mut app = app_with(d);
        assert!(report_facts(&mut app, None).unwrap_err().contains("client figures"), "structure only: refused");
        // a store file of its own, so the bank's policy can allow figures
        let dir = std::env::temp_dir().join(format!("quire_report_facts_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        app.store_path = Some(dir.join("store.json"));
        let mut st = auth::seed();
        st.ai.level = "aggregates".into();
        app.save_store(&st).unwrap();
        let (facts, m, _) = report_facts(&mut app, None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let client = app.doc.as_ref().unwrap().params.values.get("client").cloned().unwrap_or_default();
        if std::env::var("SHOW_FACTS").is_ok() { eprintln!("{facts}"); }
        assert!(facts.contains("PAGE ") && facts.contains("KPI «"), "figures, page by page");
        assert!(!client.is_empty() && !facts.contains(&client), "the client's name stays here");
        for leak in ["400PX0F", "510IIS7", "INS-777120", "***", "Газпром", "Хэндерсон", "Сбербанк"] {
            assert!(!facts.contains(leak), "{leak} is masked");
        }
        assert!(facts.len() < 60_000, "{} characters", facts.len());
        assert!(m.unmask(&facts).contains(&client) || !facts.contains("[CLIENT]"));
    }

    #[test]
    fn studio_shows_each_view_whole_and_can_add_views() {
        let mut app = app_with(theme_studio::studio_doc(theme_studio::builtin("alpine").unwrap(), vec![]));
        let views: usize = app.doc.as_ref().unwrap().theme.styles.values().map(|l| l.len()).sum();
        let st = app.resp();
        // one page per design view, each holding just its preview
        assert_eq!(st["doc"]["pages"].as_array().unwrap().len(), views);
        assert!(st["doc"]["pages"].as_array().unwrap().iter().all(|p| p["widgets"].as_array().unwrap().len() == 1));
        // tables and text are sized to show everything
        let doc = app.doc.as_ref().unwrap();
        for p in st["pages"].as_array().unwrap() {
            assert!(p["issues"].as_array().unwrap().iter().all(|i| i["kind"] != "table_rows" && i["kind"] != "text_clipped"), "{}", p["issues"]);
        }
        let n = doc.theme.styles["kpi"].len();
        run_edit(&mut app, serde_json::from_value(json!({"op": "add_variant", "kind": "kpi", "name": "Board figure", "from": null, "widget": null})).unwrap()).unwrap();
        assert_eq!(app.doc.as_ref().unwrap().theme.styles["kpi"].len(), n + 1);
    }

    #[test]
    fn contents_follow_headings_and_hidden_pages() {
        let doc = uhnwi_fixed();
        let toc_page = doc.pages.iter().position(|p| p.widgets.iter().any(|w| w.kind == "toc")).unwrap();
        let toc_id = doc.pages[toc_page].widgets.iter().find(|w| w.kind == "toc").unwrap().id.clone();
        let mut app = app_with(doc);
        let entries = |app: &mut App| -> Vec<Json> {
            let r = app.resp();
            r["pages"][toc_page]["widgets"].as_array().unwrap().iter().find(|w| w["id"] == json!(toc_id)).unwrap()["toc"].as_array().unwrap().iter().filter(|e| e["kind"] == "page").cloned().collect()
        };
        let before = entries(&mut app);
        assert!(!before.is_empty());
        // titles from each page's main heading
        run_edit(&mut app, Edit::UpdateWidget { id: toc_id.clone(), patch: json!({"opts": {"toc": {"titles": "headings"}}}) }).unwrap();
        let first = before[0]["index"].as_u64().unwrap() as usize;
        let expect = {
            let doc = app.doc.as_ref().unwrap();
            vm::page_heading(doc, &doc.pages[first]).map(|h| h.0).unwrap_or_else(|| doc.pages[first].title.clone())
        };
        assert_eq!(entries(&mut app)[0]["text"], json!(expect));
        // hiding two pages is one step, and one undo brings both back
        let hidden0: Vec<bool> = app.doc.as_ref().unwrap().pages.iter().map(|p| p.toc_hidden).collect();
        let ids: Vec<String> = before.iter().take(2).map(|e| app.doc.as_ref().unwrap().pages[e["index"].as_u64().unwrap() as usize].id.clone()).collect();
        let hide = |id: &str| serde_json::from_value::<Edit>(json!({"op": "set_page_props", "page": id, "group": null, "label": null, "frame": null, "toc_hidden": true})).unwrap();
        let n = app.undo.len();
        run_edit(&mut app, Edit::Batch { edits: ids.iter().map(|i| hide(i)).collect() }).unwrap();
        assert_eq!(app.undo.len(), n + 1);
        assert_eq!(entries(&mut app).len(), before.len() - 2);
        let prev = app.undo.pop().unwrap();
        app.doc = Some(prev);
        assert_eq!(app.doc.as_ref().unwrap().pages.iter().map(|p| p.toc_hidden).collect::<Vec<_>>(), hidden0);
    }

    #[test]
    fn sql_datasets_lod_and_parameters() {
        let mut doc = sample::template();
        // a SQL dataset over another one: a CTE, a join and a window over the report period
        let q = Dataset {
            id: "q".into(),
            name: "By region".into(),
            source: model::Source::Query { sql: "with p as (select region, sum(revenue) as rev from sales where date between :start and :end group by region) \
                select p.region, p.rev, p.rev / sum(p.rev) over () as share, t.target from p left join targets t on t.region = p.region".into() },
            ..Default::default()
        };
        doc.datasets.push(Dataset {
            id: "targets".into(),
            name: "Targets".into(),
            source: model::Source::Inline { columns: vec!["region".into(), "target".into()], rows: vec![vec![json!("North"), json!(1)], vec![json!("South"), json!(2)]] },
            ..Default::default()
        });
        // a level-of-detail field: each row's region total within the period
        doc.datasets[0].calc_fields.push(model::CalcField { name: "region_rev".into(), expr: "sum(case when date between :start and :end then revenue end) over (partition by region)".into() });
        let mut app = app_with(doc);
        let doc = app.doc.clone().unwrap();
        let t = app.engine.table(&doc, &q, None, &mut app.py).expect("sql dataset runs");
        assert_eq!(t.columns, vec!["region", "rev", "share", "target"]);
        let share: f64 = t.rows.iter().map(|r| r[2].num().unwrap_or(0.0)).sum();
        assert!((share - 1.0).abs() < 1e-9, "shares add up: {share}");
        let sales = app.engine.table(&doc, &doc.datasets[0], None, &mut app.py).unwrap();
        assert!(sales.col("region_rev").is_some());
        // a parameter that filters by column, with two values
        let mut doc2 = doc.clone();
        doc2.param_defs[0].field = "region".into();
        doc2.param_defs[0].multi = true;
        doc2.params.values.insert("region".into(), "North\u{1f}South".into());
        let all = std::sync::Arc::new(sales.as_ref().clone());
        let f = query::param_filtered(&doc2, &doc2.datasets[0], all.clone());
        assert!(f.rows.len() < all.rows.len() && !f.rows.is_empty());
        let rc = f.col("region").unwrap();
        assert!(f.rows.iter().all(|r| ["North", "South"].contains(&r[rc].text().as_str())));
        // and in SQL, a list: region in (:region)
        assert_eq!(query::bind_report("region in (:region)", &doc2), "region in ('North', 'South')");
    }

    #[test]
    fn app_settings_formats_reach_documents() {
        let app_f = model::Formats { thousands: ",".into(), ..Default::default() };
        // a Russian document that sets nothing follows the app: comma thousands, so a dot for decimals
        let f = model::Formats::default().over(&app_f, "ru-RU");
        assert_eq!((f.thousands.as_str(), f.decimal.as_str()), (",", "."));
        // the document's own choice wins
        let own = model::Formats { thousands: "space".into(), ..Default::default() }.over(&app_f, "ru-RU");
        assert_eq!(own.thousands, "space");
        // numbers the engine writes into text follow them
        period::set_locale("ru-RU");
        period::set_formats(&f);
        assert_eq!(data::fmt_num_locale(1234567.0), "1,234,567");
        assert_eq!(data::fmt_num_locale(1234.5), "1,234.50");
        period::set_formats(&model::Formats { negative: "paren".into(), ..Default::default() });
        assert_eq!(data::fmt_num_locale(-1500.0), "(1\u{a0}500)");
        period::set_formats(&model::Formats::default());
    }

    #[test]
    fn an_old_dot_scale_opens_as_a_range_bar() {
        // a file saved when dot scales were their own widget kind
        let mut doc = uhnwi_fixed();
        let views = doc.theme.styles.get_mut("bar").unwrap();
        let mut old: Vec<_> = views.iter().filter(|v| v.opts["orientation"] == "scale").cloned().collect();
        views.retain(|v| v.opts["orientation"] != "scale");
        for v in &mut old {
            v.opts.as_object_mut().unwrap().remove("orientation");
        }
        // one old view shares an id with a bar view and must be renamed
        old.push(model::StyleVariant { id: "ranked".into(), name: "Old".into(), opts: json!({}), ..Default::default() });
        doc.theme.styles.insert("scale".into(), old);
        let w = doc.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()).find(|w| w.title == "Позиция по доходности").unwrap();
        w.kind = "scale".into();
        w.opts.as_object_mut().unwrap().remove("orientation");
        let w2 = doc.pages.iter_mut().flat_map(|p| p.widgets.iter_mut()).find(|w| w.title == "Сравнение с ориентирами").unwrap();
        (w2.kind, w2.variant) = ("scale".into(), "ranked".into());
        let doc: Doc = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        let mut doc = doc;
        doc.normalize();
        assert!(!doc.theme.styles.contains_key("scale"));
        let bars = &doc.theme.styles["bar"];
        assert!(bars.iter().any(|v| v.id == "bench" && v.opts["orientation"] == "scale"));
        assert!(bars.iter().any(|v| v.id == "range-ranked" && v.opts["orientation"] == "scale"));
        let all: Vec<_> = doc.pages.iter().flat_map(|p| p.widgets.iter()).collect();
        assert!(all.iter().all(|w| w.kind != "scale"));
        let w = all.iter().find(|w| w.title == "Позиция по доходности").unwrap();
        assert_eq!((w.kind.as_str(), w.opts["orientation"].as_str()), ("bar", Some("scale")));
        let w2 = all.iter().find(|w| w.title == "Сравнение с ориентирами").unwrap();
        assert_eq!(w2.variant, "range-ranked", "the renamed view follows");
    }

    #[test]
    fn a_user_sees_and_reports_only_their_clients() {
        if dbconn::demo_path().is_none() {
            return;
        }
        let mut doc = uhnwi_fixed();
        to_report(&mut doc, None);
        let mut app = app_with(doc);
        let store = auth::seed();
        // nobody signed in: nothing is allowed
        assert!(gate(&app, &[]).is_err());
        // the banker covers six clients
        let s = auth::sign_in(&store, "user", auth::DEMO_PASSWORD).unwrap();
        app.start_session(s).unwrap();
        assert_eq!(app.clients.as_ref().unwrap().len(), 6);
        assert!(gate(&app, doc_roles(app.doc.as_ref())).is_ok(), "a user works on reports");
        assert!(gate(&app, &["design"]).is_err() && gate(&app, &["data"]).is_err() && gate(&app, &["admin"]).is_err());
        let r = app.resp();
        let opts = r["param_options"]["client"].as_array().unwrap();
        assert_eq!(opts.len(), 6, "the client picker lists only their clients");
        assert!(opts.iter().any(|v| v == uhnwi_sample::TEST_CLIENT));
        // another banker's client is refused
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.values.insert("client".into(), "Stephen Curryman".into());
        assert!(check_access_edit(&app, &Edit::SetParams { params: p.clone() }).is_err());
        // and a report that names one moves to one of theirs
        app.doc.as_mut().unwrap().params.values.insert("client".into(), "Stephen Curryman".into());
        let r = app.resp();
        let now = r["doc"]["params"]["values"]["client"].as_str().unwrap().to_string();
        assert!(opts.iter().any(|v| v == &now), "moved to {now}");
        // a template opened for design needs the Design role
        let tpl = uhnwi_fixed();
        assert!(gate_open(&app, &tpl).is_err());
        // an admin sees everyone
        app.end_session();
        let s = auth::sign_in(&store, "admin", auth::DEMO_PASSWORD).unwrap();
        app.start_session(s).unwrap();
        assert!(app.clients.is_none());
        assert!(gate(&app, &["design"]).is_ok() && gate(&app, &["data"]).is_ok() && gate_open(&app, &tpl).is_ok());
    }

    #[test]
    fn python_reads_other_sources_and_runs_on_a_result() {
        let mut py = PyEngine::new();
        if py.info().get("error").is_some() {
            return; // no Python here
        }
        let mut doc = sample::blank();
        doc.datasets.push(client_sample::inline("a", "Суммы", &["k", "v"], vec![vec![json!("x"), json!(2.0)], vec![json!("y"), json!(5.0)]], None, false));
        let code = "src = sources[\"Суммы\"]\nrows = src.to_dict('records') if hasattr(src, 'to_dict') else src\n[{'k': r['k'], 'double': r['v'] * 2} for r in rows]";
        doc.datasets.push(Dataset { id: "p".into(), name: "Удвоено".into(), source: model::Source::Python { code: code.into() }, ..Default::default() });
        let mut engine = Engine::default();
        let ds = doc.dataset("p").unwrap().clone();
        let t = engine.table_all(&doc, &ds, None, &mut py).unwrap();
        assert_eq!(t.columns, vec!["k", "double"]);
        assert_eq!(t.rows[1][1].num(), Some(10.0));
        // the SQL workspace's Python: `df` is the last result
        let r = py.run_on("print('rows', len(df))\nlen(df) * 10", &json!({"columns": ["a"], "rows": [[1], [2], [3]]}), &json!({}));
        assert!(r.ok, "{:?}", r.error);
        assert!(r.stdout.contains("rows 3"));
        assert_eq!(r.value["table"]["rows"][0][0].as_f64().or_else(|| r.value["text"].as_str().and_then(|t| t.parse().ok())), Some(30.0));
    }

    #[test]
    fn the_products_filter_reaches_every_source() {
        let doc = uhnwi_fixed();
        // every source follows both parameters (only the lists of choices stand apart)
        for name in ["client", "product"] {
            let r = query::param_reach(&doc, name);
            let not: Vec<&str> = r["not"].as_array().unwrap().iter().filter_map(|x| x["name"].as_str()).collect();
            assert!(not.is_empty(), "{name}: these sources don't follow it: {not:?}");
        }
        if dbconn::demo_path().is_none() {
            return;
        }
        // brokerage only: the portfolio is the brokerage accounts', the bank, DU and policy pages go
        let mut app = app_with(uhnwi_fixed());
        let r = app.resp();
        assert_eq!(r["param_labels"]["product"]["BROK"], "Брокерские счета");
        let all_value = app.engine.table_all(app.doc.as_ref().unwrap(), app.doc.as_ref().unwrap().dataset("port").unwrap(), None, &mut app.py).unwrap().rows[0][0].num().unwrap();
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.values.insert("product".into(), "BROK".into());
        run_edit(&mut app, Edit::SetParams { params: p }).unwrap();
        let d = app.doc.as_ref().unwrap();
        let t = app.engine.table_all(d, d.dataset("port").unwrap(), None, &mut app.py).unwrap();
        let brok_value = t.rows[0][t.col("value").unwrap()].num().unwrap();
        assert!(brok_value < all_value && brok_value > 0.0, "{brok_value} of {all_value}");
        let r = app.resp();
        let shown: Vec<String> = r["pages"].as_array().unwrap().iter().filter(|p| !p["hidden"].as_bool().unwrap_or(false)).map(|p| p["title"].as_str().unwrap_or("").to_string()).collect();
        assert!(!shown.iter().any(|t| t.contains("Банковские") || t.contains("Доверительное")), "{shown:?}");
        assert!(shown.iter().any(|t| t.contains("400PX0F")), "{shown:?}");
        // several at once
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.values.insert("product".into(), "BROK\u{1f}BANK".into());
        run_edit(&mut app, Edit::SetParams { params: p }).unwrap();
        let r = app.resp();
        assert!(r["pages"].as_array().unwrap().iter().any(|p| !p["hidden"].as_bool().unwrap_or(false) && p["title"].as_str().unwrap_or("").contains("Банковские")));
    }

    #[test]
    fn an_apostrophe_in_an_sql_comment_leaves_parameters_alone() {
        let mut v = std::collections::BTreeMap::new();
        v.insert("client".to_string(), "Кирилл".to_string());
        let (q, _) = data::bind_sql_named("select 1 -- the client's name\nwhere name = :client and x = 'it''s'", None, &v);
        assert!(q.contains("name = 'Кирилл'"), "{q}");
        assert!(q.contains("-- the client's name"));
    }

    #[test]
    fn widget_types_are_saved_reused_updated_and_detached() {
        let dir = std::env::temp_dir().join(format!("quire_wtypes_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = app_with(uhnwi_sample::template());
        app.store_path = Some(dir.join("users.json"));
        let st = app.store();
        app.start_session(auth::sign_in(&st, "design", auth::DEMO_PASSWORD).unwrap()).unwrap();
        let doc = app.doc.as_ref().unwrap();
        let kpi = doc.pages.iter().flat_map(|p| &p.widgets).find(|w| w.kind == "kpi" && w.binding.is_some()).unwrap().clone();
        let page = doc.pages.iter().find(|p| p.widgets.iter().any(|w| w.id == kpi.id)).unwrap().id.clone();
        let before = model::effective(&doc.theme, &kpi).into_owned();
        // saved as a type: the widget follows it and looks the same
        let r = auth_cmd(&mut app, "widget_type_save", &json!({"widget": kpi.id, "name": "Итог · карточка", "icon": "wallet"})).unwrap();
        let tid = r["type"]["id"].as_str().unwrap().to_string();
        let doc = app.doc.as_ref().unwrap();
        let w = doc.pages.iter().flat_map(|p| &p.widgets).find(|w| w.id == kpi.id).unwrap();
        assert_eq!(w.wtype, tid);
        assert_eq!(model::effective(&doc.theme, w).opts, before.opts);
        assert_eq!(auth_cmd(&mut app, "widget_types", &json!({})).unwrap()[0]["used_here"], json!(1));
        // a second widget made from it; then the type changes and both follow
        auth_cmd(&mut app, "add_typed_widget", &json!({"id": tid, "page": page})).unwrap();
        run_edit(&mut app, Edit::UpdateWidget { id: kpi.id.clone(), patch: json!({"opts": {"viz": {"kpiAlign": "right"}}}) }).unwrap();
        let r = auth_cmd(&mut app, "widget_type_save", &json!({"widget": kpi.id, "update": true})).unwrap();
        assert_eq!(r["type"]["version"], json!(2));
        let doc = app.doc.as_ref().unwrap();
        let typed: Vec<&Widget> = doc.pages.iter().flat_map(|p| &p.widgets).filter(|w| w.wtype == tid).collect();
        assert_eq!(typed.len(), 2);
        assert!(typed.iter().all(|w| model::effective(&doc.theme, w).opts["viz"]["kpiAlign"] == "right"), "both follow the type");
        let other = typed.iter().find(|w| w.id != kpi.id).unwrap().id.clone();
        // another theme keeps the types
        apply_theme_to(&mut app, "builtin:alpine").unwrap();
        assert!(app.doc.as_ref().unwrap().theme.widget_types.contains_key(&tid));
        // detached: keeps the look, no longer follows
        run_edit(&mut app, Edit::DetachWidgetType { id: other.clone() }).unwrap();
        let doc = app.doc.as_ref().unwrap();
        let d = doc.pages.iter().flat_map(|p| &p.widgets).find(|w| w.id == other).unwrap();
        assert!(d.wtype.is_empty() && model::effective(&doc.theme, d).opts["viz"]["kpiAlign"] == "right");
        // a template with an older copy takes the library's newer version when it opens
        let mut old = app.doc.as_ref().unwrap().clone();
        old.theme.widget_types.get_mut(&tid).unwrap().version = 1;
        old.theme.widget_types.get_mut(&tid).unwrap().opts = json!({});
        app.load(old, None);
        assert_eq!(app.doc.as_ref().unwrap().theme.widget_types[&tid].version, 2);
        // deleted from the library: the template keeps its copy
        auth_cmd(&mut app, "widget_type_delete", &json!({"id": tid})).unwrap();
        assert!(auth_cmd(&mut app, "widget_types", &json!({})).unwrap().as_array().unwrap().is_empty());
        assert!(app.doc.as_ref().unwrap().theme.widget_types.contains_key(&tid));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Deleting a template takes its file and its published report away from the list and from report users —
    /// into the library's ".deleted" folder, not erased.
    #[test]
    fn deleting_a_template_puts_it_and_its_report_away() {
        let dir = std::env::temp_dir().join(format!("quire_del_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = app_with(uhnwi_sample::template());
        app.store_path = Some(dir.join("users.json"));
        let st = app.store();
        app.start_session(auth::sign_in(&st, "design", auth::DEMO_PASSWORD).unwrap()).unwrap();
        // a template in the library, published
        let lib = templates_dir(&app).unwrap();
        let file = lib.join("Old report.rtpl");
        app.doc.as_mut().unwrap().meta.report_id = String::new();
        app.doc.as_mut().unwrap().meta.title = "Old report".into();
        auth_cmd(&mut app, "publish", &json!({})).unwrap();
        std::fs::write(&file, serde_json::to_string(app.doc.as_ref().unwrap()).unwrap()).unwrap();
        let rid = app.doc.as_ref().unwrap().meta.report_id.clone();
        let home = auth_cmd(&mut app, "design_home", &json!({})).unwrap();
        assert!(home["templates"].as_array().unwrap().iter().any(|t| t["title"] == "Old report"));
        // deleted: gone from the list and from the published reports, both kept in .deleted
        let r = auth_cmd(&mut app, "delete_template", &json!({"path": file.display().to_string(), "report": rid})).unwrap();
        assert_eq!(r["deleted"], json!(["template", "report"]));
        assert!(!file.exists());
        assert!(!app.store().reports.iter().any(|x| x.id == rid), "report users lose it");
        let home = auth_cmd(&mut app, "design_home", &json!({})).unwrap();
        assert!(!home["templates"].as_array().unwrap().iter().any(|t| t["title"] == "Old report"));
        let kept: Vec<_> = std::fs::read_dir(lib.join(".deleted")).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(kept.len(), 2, "the template and its published copy, recoverable");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_widget_studio_makes_and_edits_widgets_on_sample_data() {
        let dir = std::env::temp_dir().join(format!("quire_wstudio_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = app_with(sample::blank());
        app.store_path = Some(dir.join("users.json"));
        let st = app.store();
        app.start_session(auth::sign_in(&st, "design", auth::DEMO_PASSWORD).unwrap()).unwrap();
        // the gallery draws every widget: the theme's views
        let g = auth_cmd(&mut app, "widget_gallery", &json!({})).unwrap();
        let items = g["items"].as_array().unwrap();
        assert!(items.len() > 20);
        // each built-in widget once: the Quire theme's waterfall is the shared one, listed with both themes
        let wf: Vec<&Json> = items.iter().filter(|i| i["kind"] == "waterfall" && i["group"] == "builtin").collect();
        assert_eq!(wf.len(), 1);
        assert!(wf[0]["themes"].as_array().unwrap().iter().any(|t| t == "Quire") && wf[0]["themes"].as_array().unwrap().len() > 1);
        // names in the preview theme's language, the Quire-only widgets too
        assert!(items.iter().any(|i| i["view"] == "toned" && i["name"] == "Тонированная карточка"));
        // a built-in widget opened, changed and saved as one's own: no sample data goes with it
        auth_cmd(&mut app, "widget_open", &json!({"kind": "kpi", "view": "breakdown"})).unwrap();
        assert_eq!(app.doc.as_ref().unwrap().widget_studio, "new");
        assert!(!app.dirty);
        run_edit(&mut app, Edit::UpdateWidget { id: "w_studio".into(), patch: json!({"opts": {"viz": {"kpiAlign": "center"}}}) }).unwrap();
        let r = auth_cmd(&mut app, "widget_type_save", &json!({"widget": "w_studio", "name": "Моя карточка"})).unwrap();
        let tid = r["type"]["id"].as_str().unwrap().to_string();
        assert!(r["type"]["binding"].is_null() && r["type"]["title"] == "");
        assert_eq!(r["type"]["variant"], "breakdown");
        assert_eq!(r["type"]["opts"]["viz"]["kpiAlign"], "center");
        let d = app.doc.as_ref().unwrap();
        assert_eq!(d.widget_studio, tid);
        assert!(!app.dirty);
        // opened again from the library and updated: version 2, still no sample data
        auth_cmd(&mut app, "widget_open", &json!({"type": tid})).unwrap();
        run_edit(&mut app, Edit::UpdateWidget { id: "w_studio".into(), patch: json!({"opts": {"viz": {"arrow": true}}}) }).unwrap();
        let r = auth_cmd(&mut app, "widget_type_save", &json!({"widget": "w_studio", "update": true})).unwrap();
        assert_eq!((r["type"]["version"].as_u64(), r["type"]["opts"]["viz"]["arrow"].as_bool()), (Some(2), Some(true)));
        assert!(r["type"]["binding"].is_null());
        let g = auth_cmd(&mut app, "widget_gallery", &json!({})).unwrap();
        assert!(g["items"].as_array().unwrap().iter().any(|i| i["type"] == tid.as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_template_is_duplicated_into_another_theme() {
        let dir = std::env::temp_dir().join(format!("quire_dup_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = app_with(sample::blank());
        app.store_path = Some(dir.join("users.json"));
        let st = app.store();
        app.start_session(auth::sign_in(&st, "design", auth::DEMO_PASSWORD).unwrap()).unwrap();
        // the published report's template, copied under a new name into Bold Blocks
        auth_cmd(&mut app, "duplicate_template", &json!({"report": "uhnwi", "title": "Отчёт · блоки", "theme": "builtin:blocks"})).unwrap();
        let d = app.doc.as_ref().unwrap();
        assert_eq!((d.meta.title.as_str(), d.theme.name.as_str(), d.meta.report_id.as_str()), ("Отчёт · блоки", uhnwi_theme::BLOCKS.name, ""));
        // saved into the templates library straight away
        let path = app.path.clone().unwrap();
        assert!(!app.dirty && app.undo.is_empty() && path.starts_with(dir.join("templates")) && path.exists());
        // the start page lists it as a draft, beside the published report (with its theme)
        let home = auth_cmd(&mut app, "design_home", &json!({})).unwrap();
        assert!(home["reports"][0]["theme"].as_str().is_some_and(|t| !t.is_empty()));
        let row = |h: &Json| h["templates"].as_array().unwrap().iter().find(|t| t["title"] == "Отчёт · блоки").cloned().unwrap();
        assert!(row(&home)["report"].is_null() && row(&home)["in_library"] == true);
        // published: the file keeps its link, nothing changed since
        auth_cmd(&mut app, "publish", &json!({})).unwrap();
        let home = auth_cmd(&mut app, "design_home", &json!({})).unwrap();
        assert_eq!((row(&home)["report"]["version"].as_u64(), row(&home)["changed"].as_bool()), (Some(1), Some(false)));
        // a page added and saved: changed since publishing
        run_edit(&mut app, Edit::AddPage { after: None }).unwrap();
        save_template_here(&mut app).unwrap();
        let home = auth_cmd(&mut app, "design_home", &json!({})).unwrap();
        assert_eq!(row(&home)["changed"], json!(true));
        assert!(auth_cmd(&mut app, "duplicate_template", &json!({"kind": "uhnwi_report", "title": " "})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_go_to_their_audience() {
        let dir = std::env::temp_dir().join(format!("quire_publish_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = app_with(sample::blank());
        app.store_path = Some(dir.join("users.json"));
        let st = app.store();
        let sign = |app: &mut App, l: &str| {
            app.end_session();
            let st = app.store();
            app.start_session(auth::sign_in(&st, l, auth::DEMO_PASSWORD).unwrap()).unwrap();
        };
        // the built-in report goes to the private bankers: Warren is one
        sign(&mut app, "user");
        assert!(report_from_published(&app, "uhnwi").is_ok());
        let mine = auth_cmd(&mut app, "reports", &json!({})).unwrap();
        assert_eq!(mine.as_array().unwrap().len(), 1);
        assert!(mine[0].get("audience").is_none(), "report users don't see the audience");
        // a designer publishes a new template to the insurance desk only
        sign(&mut app, "design");
        app.load(sample::blank(), None);
        app.doc.as_mut().unwrap().meta.title = "Policy summary".into();
        let r = auth_cmd(&mut app, "publish", &json!({"audience": {"everyone": false, "groups": ["insurance"], "users": []}})).unwrap();
        let id = r["report"]["id"].as_str().unwrap().to_string();
        assert_eq!(app.doc.as_ref().unwrap().meta.report_id, id, "the template knows it is published");
        assert!(dir.join("reports").join(format!("{id}.rtpl")).exists());
        // Warren isn't on the insurance desk…
        sign(&mut app, "user");
        assert!(report_from_published(&app, &id).is_err());
        // …until an admin adds him
        sign(&mut app, "admin");
        auth_cmd(&mut app, "set_user_groups", &json!({"login": "user", "groups": ["bankers", "insurance"]})).unwrap();
        sign(&mut app, "user");
        let d = report_from_published(&app, &id).unwrap();
        assert_eq!(d.kind, DocKind::Report);
        // a draft is nobody's
        sign(&mut app, "design");
        auth_cmd(&mut app, "set_audience", &json!({"id": id, "published": false})).unwrap();
        sign(&mut app, "user");
        assert!(report_from_published(&app, &id).is_err());
        // the Data role reads a report's own sources without opening it
        sign(&mut app, "data");
        let rd = auth_cmd(&mut app, "report_data", &json!({"id": "uhnwi"})).unwrap();
        let n0 = rd["datasets"].as_array().unwrap().len();
        assert!(n0 > 30);
        // …and adds one to it
        let src = json!({"id": "", "name": "Новый источник", "source": {"type": "inline", "columns": ["a"], "rows": [[1]]}});
        let added = auth_cmd(&mut app, "save_report_source", &json!({"id": "uhnwi", "dataset": src})).unwrap()["id"].as_str().unwrap().to_string();
        let rd = auth_cmd(&mut app, "report_data", &json!({"id": "uhnwi", "peek": true})).unwrap();
        assert_eq!(rd["datasets"].as_array().unwrap().len(), n0 + 1);
        assert_eq!(rd["changed"][0], json!(added));
        // the designer finds it in the template, and publishing takes it along
        sign(&mut app, "design");
        app.load(uhnwi_fixed(), None);
        assert!(app.doc.as_ref().unwrap().datasets.iter().any(|d| d.id == added));
        auth_cmd(&mut app, "publish", &json!({})).unwrap();
        assert!(app.store().report_sources.get("uhnwi").is_none(), "folded into the published pages");
        assert!(published_doc(&app, "uhnwi").unwrap().datasets.iter().any(|d| d.id == added));
        let _ = st;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_sources_are_made_in_data_and_used_in_templates() {
        let dir = std::env::temp_dir().join(format!("quire_shared_src_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = app_with(sample::blank());
        app.store_path = Some(dir.join("users.json"));
        let st = app.store();
        if st.datasets.is_empty() {
            return; // no demo database here
        }
        app.start_session(auth::sign_in(&st, "admin", auth::DEMO_PASSWORD).unwrap()).unwrap();
        // a designer adds a shared source: its copy and its connection come along
        auth_cmd(&mut app, "use_source", &json!({"id": "s_clients"})).unwrap();
        let d = app.doc.as_ref().unwrap();
        assert!(d.datasets.iter().any(|x| x.id == "s_clients" && x.shared));
        assert!(d.connections.iter().any(|c| c.id == "demo"));
        // the template can't change it…
        let mut changed = d.datasets.iter().find(|x| x.id == "s_clients").unwrap().clone();
        changed.name = "Mine".into();
        assert!(check_access_edit(&app, &Edit::UpsertDataset { dataset: changed.clone() }).is_err());
        // …the Data area can, and the open template follows
        let r = auth_cmd(&mut app, "save_source", &json!({"dataset": changed})).unwrap();
        assert_eq!(r["id"], "s_clients");
        assert_eq!(app.doc.as_ref().unwrap().datasets.iter().find(|x| x.id == "s_clients").unwrap().name, "Mine");
        // a new one gets its own id
        let mut n = changed.clone();
        n.id = String::new();
        n.name = "Клиенты 2".into();
        let r = auth_cmd(&mut app, "save_source", &json!({"dataset": n})).unwrap();
        assert!(r["id"].as_str().unwrap().starts_with("s_"));
        // deleted in Data: the template keeps its own copy
        auth_cmd(&mut app, "delete_source", &json!({"id": "s_clients"})).unwrap();
        let d = app.doc.as_ref().unwrap();
        assert!(d.datasets.iter().any(|x| x.id == "s_clients" && !x.shared));
        assert!(app.store().datasets.iter().all(|x| x.id != "s_clients"));
        // usage is recorded for a saved template
        auth_cmd(&mut app, "use_source", &json!({"id": "s_accounts"})).unwrap();
        app.path = Some(dir.join("t.rtpl"));
        let mut st = app.store();
        assert!(app.record_usage(&mut st));
        assert_eq!(st.usage["s_accounts"][0].path, dir.join("t.rtpl").display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn designers_take_connections_from_the_data_area() {
        let mut app = app_with(sample::template());
        let store = auth::seed();
        app.start_session(auth::sign_in(&store, "design", auth::DEMO_PASSWORD).unwrap()).unwrap();
        let mine = model::Connection { id: "x1".into(), name: "My DWH".into(), kind: "postgres".into(), path: String::new(), url: "postgresql://me:pw@host/db".into() };
        let set = |c: Vec<model::Connection>| Edit::SetConnections { connections: c };
        assert!(check_access_edit(&app, &set(vec![mine.clone()])).is_err(), "a designer can't bring their own credentials");
        if let Some(shared) = store.connections.first() {
            assert!(check_access_edit(&app, &set(vec![shared.clone()])).is_ok(), "a shared connection is fine");
        }
        assert!(check_access_edit(&app, &set(vec![])).is_ok(), "removing is fine");
        app.end_session();
        app.load(sample::template(), None);
        app.start_session(auth::sign_in(&store, "data", auth::DEMO_PASSWORD).unwrap()).unwrap();
        assert!(check_access_edit(&app, &set(vec![mine])).is_ok(), "the Data role manages connections");
    }

    #[test]
    fn an_insurance_user_sees_only_insurance_accounts() {
        let Some(path) = dbconn::demo_path() else { return };
        let conn = duckdb::Connection::open_with_flags(&path, duckdb::Config::default().access_mode(duckdb::AccessMode::ReadOnly).unwrap()).unwrap();
        // a client with a policy and other accounts open in the period
        let client: String = conn
            .query_row("select c.name from clients c where exists (select 1 from accounts a where a.client_id = c.id and a.src_cd = 'INS' and a.close_dt >= date '2026-01-01')
                        and exists (select 1 from accounts a where a.client_id = c.id and a.src_cd = 'BROK' and a.close_dt >= date '2026-01-01') order by c.name limit 1", [], |r| r.get(0))
            .unwrap();
        let ins_value: f64 = conn
            .query_row("select sum(p.bal_out)::double from portf_acct p join accounts a using (acct_cd) join clients c on c.id = a.client_id
                        where c.name = ? and a.src_cd = 'INS' and p.report_dt = date '2026-06-15'", [&client], |r| r.get(0))
            .unwrap();
        let mut doc = uhnwi_fixed();
        to_report(&mut doc, None);
        doc.params.values.insert("client".into(), client.clone());
        let mut app = app_with(doc);
        let s = auth::Session { login: "ins".into(), name: "Ins".into(), roles: auth::Roles { user: true, ..Default::default() }, access: auth::Access { all: true, sources: vec!["INS".into()], ..Default::default() } };
        app.start_session(s).unwrap();
        let doc = app.doc.take().unwrap();
        app.load(doc, None);
        let r = app.resp();
        assert!(r["param_options"]["client"].as_array().unwrap().iter().any(|v| v == &json!(client)), "a client with a policy is in the list");
        let d = app.doc.as_ref().unwrap();
        // shown pages: insurance accounts repeat, brokerage and DU pages have no rows, the bank is hidden
        let shown: Vec<&Json> = r["pages"].as_array().unwrap().iter().filter(|p| !p["hidden"].as_bool().unwrap_or(false)).collect();
        let titles: Vec<String> = shown.iter().map(|p| p["title"].as_str().unwrap_or("").to_string()).collect();
        assert!(d.pages.iter().any(|p| p.repeat.values.iter().any(|v| v == "INS") && p.copies > 0), "the policy pages repeat: {titles:?}");
        assert!(!titles.iter().any(|t| t.contains("Брокерский") || t.contains("Доверительное")), "no brokerage or DU pages: {titles:?}");
        // the portfolio total is the policies' value
        let t = app.engine.table_all(d, d.dataset("port").unwrap(), None, &mut app.py).unwrap();
        let v = t.rows[0][t.col("value").unwrap()].num().unwrap();
        assert!((v - ins_value).abs() < 1.0, "portfolio {v} = policies {ins_value}");
    }

    /// The money page: its charges add up to the cards above them; each deal has its type and a result at the end.
    #[test]
    fn money_page_agrees_with_its_cards_and_deals_carry_type_and_result() {
        if dbconn::demo_path().is_none() { return }
        let mut app = app_with(uhnwi_fixed());
        let d = app.doc.as_ref().unwrap();
        let port = app.engine.table_all(d, d.dataset("port").unwrap(), None, &mut app.py).unwrap();
        let p = |k: &str| port.rows[0][port.col(k).unwrap()].num().unwrap();
        let ch = app.engine.table_all(d, d.dataset("charges").unwrap(), None, &mut app.py).unwrap();
        let sum = |kind: &str| ch.rows.iter().filter(|r| r[ch.col("kind").unwrap()].text() == kind).map(|r| r[ch.col("paid").unwrap()].num().unwrap()).sum::<f64>();
        assert!((sum("tax") - p("tax_paid")).abs() < 1.0, "taxes {} = card {}", sum("tax"), p("tax_paid"));
        assert!((sum("fee") - p("fees_paid")).abs() < 1.0, "fees {} = card {}", sum("fee"), p("fees_paid"));
        let deals = app.engine.table_all(d, d.dataset("deals").unwrap(), None, &mut app.py).unwrap();
        assert!(!deals.rows.is_empty());
        for r in &deals.rows {
            assert!(!r[deals.col("kind").unwrap()].text().is_empty(), "every deal has its instrument's type");
            assert!(r[deals.col("res_pct").unwrap()].num().is_some(), "every deal has a result at the end");
        }
        let page = d.pages.iter().position(|p| p.title.starts_with("Движение средств")).unwrap();
        assert_eq!(d.pages[page - 1].title, "Сделки по инструментам", "the money page follows the deals");
    }

    #[test]
    fn account_pages_repeat_for_each_of_the_clients_accounts() {
        let Some(path) = dbconn::demo_path() else { return };
        let conn = duckdb::Connection::open_with_flags(&path, duckdb::Config::default().access_mode(duckdb::AccessMode::ReadOnly).unwrap()).unwrap();
        // how many accounts of a kind a client had open in the period (01.01–15.06.2026)
        let count = |client: &str, kinds: &str| -> usize {
            let n: i64 = conn
                .query_row(&format!("select count(*) from accounts a join clients c on c.id = a.client_id where c.name = ? and a.src_cd in ({kinds}) and a.open_dt <= date '2026-06-15' and a.close_dt >= date '2026-01-01'"), [client], |r| r.get(0))
                .unwrap();
            n as usize
        };
        // clients worth checking: the test client, one without DU, one with two brokerage accounts, one with a policy
        let pick = |sql: &str| -> String { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
        let clients = [
            uhnwi_sample::TEST_CLIENT.to_string(),
            pick("select c.name from clients c where not exists (select 1 from accounts a where a.client_id = c.id and a.src_cd = 'AM') order by 1 limit 1"),
            pick("select c.name from clients c join accounts a on a.client_id = c.id where a.src_cd = 'BROK' and a.close_dt >= date '2026-01-01' group by 1 having count(*) = 2 order by 1 limit 1"),
            pick("select c.name from clients c join accounts a on a.client_id = c.id where a.src_cd = 'INS' and c.id <> 'C1337' order by 1 limit 1"),
        ];
        let mut app = app_with(uhnwi_fixed());
        for client in &clients {
            let mut p = app.doc.as_ref().unwrap().params.clone();
            p.values.insert("client".into(), client.clone());
            apply(&mut app, Edit::SetParams { params: p }).unwrap();
            app.sync_chains(); // as every edit in the app does
            let r = app.resp();
            let doc = app.doc.as_ref().unwrap();
            let pages = r["pages"].as_array().unwrap();
            // shown overview pages of a kind's group (the designed one and its copies)
            let shown = |group: &str| doc.pages.iter().zip(pages).filter(|(p, v)| p.label == "Обзор" && p.group.split('~').next() == Some(group) && !v["hidden"].as_bool().unwrap()).count();
            assert_eq!(shown("BROK"), count(client, "'BROK'"), "{client}: brokerage pages");
            assert_eq!(shown("DU"), count(client, "'AM'"), "{client}: DU pages");
            assert_eq!(shown("INS"), count(client, "'INS'"), "{client}: insurance pages");
            // shown pages are numbered without gaps, and every title is filled in
            let numbers: Vec<u64> = pages.iter().filter(|v| !v["hidden"].as_bool().unwrap()).map(|v| v["number"].as_u64().unwrap()).collect();
            assert_eq!(numbers, (1..=numbers.len() as u64).collect::<Vec<_>>(), "{client}");
            let toc = vm::toc(doc, None);
            assert!(toc.iter().all(|e| !e.text.contains("{{") && !e.description.contains("{{")), "{client}: {:?}", toc.iter().map(|e| &e.text).collect::<Vec<_>>());
            // each copy shows its own account
            let copies: Vec<&model::Page> = doc.pages.iter().filter(|p| p.group.starts_with("BROK") && p.label == "Обзор" && !p.row.is_empty()).collect();
            let accts: std::collections::HashSet<&str> = copies.iter().map(|p| p.row["acct_cd"].as_str()).collect();
            assert_eq!(accts.len(), copies.len(), "{client}: every brokerage copy has its own account");
        }
        // the test client: 400PX0F and its IIS, each with its own holdings
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.values.insert("client".into(), uhnwi_sample::TEST_CLIENT.into());
        apply(&mut app, Edit::SetParams { params: p }).unwrap();
        app.sync_chains();
        let toc: Vec<String> = vm::toc(app.doc.as_ref().unwrap(), None).into_iter().map(|e| e.text).collect();
        assert!(toc.iter().any(|t| t == "Брокерский счёт 400PX0F") && toc.iter().any(|t| t == "ИИС 510IIS7"), "{toc:?}");
        assert!(toc.iter().any(|t| t.contains("Семейный резерв")), "{toc:?}");
        // a designed continuation page with no rows left (the IIS's few holdings fit on one page) is left out
        let r = app.resp();
        let shown: Vec<(String, bool)> = r["pages"].as_array().unwrap().iter().map(|p| (p["title"].as_str().unwrap().to_string(), p["hidden"].as_bool().unwrap())).collect();
        let cont = |t: &str| shown.iter().find(|(x, _)| x.starts_with(t) && x.contains("(продолжение)")).map(|(_, h)| *h);
        assert_eq!(cont("Брокерский счёт 400PX0F"), Some(false), "{shown:?}");
        assert_eq!(cont("ИИС 510IIS7"), Some(true), "{shown:?}");
    }

    #[test]
    fn a_continued_table_given_room_takes_its_page_back() {
        if dbconn::demo_path().is_none() {
            return;
        }
        let mut app = app_with(uhnwi_fixed());
        app.sync_chains();
        let find = |app: &App| -> (String, usize) {
            let doc = app.doc.as_ref().unwrap();
            let p = doc.pages.iter().find(|p| p.title.starts_with("Доверительное управление · состав") && p.repeat_of.is_empty()).unwrap();
            let t = p.widgets.iter().find(|w| w.kind == "table").unwrap();
            let pieces = doc.pages.iter().flat_map(|p| &p.widgets).filter(|w| vm::continues(w) == Some(t.id.as_str())).count();
            (t.id.clone(), pieces)
        };
        let (id, before) = find(&app);
        assert!(before >= 1, "the DU holdings continue");
        // room for everything: the table takes the page below it
        let page = app.doc.as_ref().unwrap().pages.iter().position(|p| p.widgets.iter().any(|w| w.id == id)).unwrap();
        let others: Vec<String> = app.doc.as_ref().unwrap().pages[page].widgets.iter().filter(|w| w.y >= 17).map(|w| w.id.clone()).collect();
        for o in others {
            run_edit(&mut app, Edit::RemoveWidget { id: o }).unwrap();
        }
        // fewer rows (the equity funds only) and every free row below: they all fit now
        let mut binding = app.doc.as_ref().unwrap().find_widget(&id).map(|(pi, wi)| app.doc.as_ref().unwrap().pages[pi].widgets[wi].binding.clone()).unwrap().unwrap();
        binding.filters.push(serde_json::from_value(json!({"field": "bucket", "op": "=", "value": "Денежная позиция"})).unwrap());
        run_edit(&mut app, Edit::UpdateWidget { id: id.clone(), patch: json!({"h": 19, "binding": binding}) }).unwrap();
        assert_eq!(find(&app).1, 0, "no continuation once every row fits");
    }

    #[test]
    fn variables_can_be_used_in_sql_as_parameters() {
        let Some(path) = dbconn::demo_path() else { return };
        let conn = model::Connection { id: "c".into(), name: "Demo".into(), kind: "duckdb".into(), path, url: String::new() };
        let mut doc = uhnwi_fixed();
        doc.variables.push(model::Variable { name: "min_value".into(), kind: "text".into(), value: "1000000".into(), ..Default::default() });
        doc.variables.push(model::Variable { name: "as_of".into(), kind: "text".into(), value: "{{end}}".into(), ..Default::default() });
        let listed = dbconn::params(&doc);
        let names: Vec<&str> = listed.as_array().unwrap().iter().filter_map(|p| p["name"].as_str()).collect();
        assert!(names.contains(&"min_value") && names.contains(&"as_of") && names.contains(&"client"), "{names:?}");
        let mut py = python::PyEngine::default();
        let r = dbconn::run(&conn, "select cast(:min_value as integer) + 1 as v, :as_of as d, :period_start as ps", 5, Some(&doc), None, &mut py).unwrap();
        assert_eq!(r["rows"][0][0], json!(1000001.0));
        assert!(r["rows"][0][1].as_str().is_some_and(|d| !d.contains("{{")), "the variable's placeholders are filled in: {}", r["rows"][0][1]);
        assert_eq!(r["rows"][0][2], json!("2026-01-01"));
        // a bare parameter is named after itself, not after its value
        let r = dbconn::run(&conn, "select :client, :period_start, :data_start, :period_end as pe", 5, Some(&doc), None, &mut py).unwrap();
        let names: Vec<&str> = r["columns"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names, [":client", ":period_start", ":data_start", "pe"]);
        assert_eq!(r["rows"][0][0], json!(uhnwi_sample::TEST_CLIENT));
    }

    #[test]
    fn edited_cells_are_saved_to_the_database_all_or_nothing() {
        let dir = std::env::temp_dir().join(format!("quire-edit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.duckdb");
        let _ = std::fs::remove_file(&path);
        {
            let c = duckdb::Connection::open(&path).unwrap();
            c.execute_batch("create table t (id integer, name varchar, d date, v decimal(10,2)); insert into t values (1, 'a', '2026-01-01', 1.5), (2, 'b', '2026-01-02', 2.5);").unwrap();
        }
        let conn = model::Connection { id: "c".into(), name: "T".into(), kind: "duckdb".into(), path: path.display().to_string(), url: String::new() };
        let mut py = python::PyEngine::default();
        let r = dbconn::run(&conn, "select rowid as __qrow, * from t order by id", 10, None, None, &mut py).unwrap();
        let key = r["rows"][1][0].clone();
        let e = |column: &str, value: Json, ty: &str| dbconn::CellEdit { key: key.clone(), column: column.into(), value, r#type: ty.into() };
        let n = dbconn::apply(&conn, "main", "t", &[e("name", json!("it's"), "VARCHAR"), e("d", json!("2026-05-31"), "DATE"), e("v", Json::Null, "DECIMAL(10,2)")], None).unwrap();
        assert_eq!(n, 3);
        let r = dbconn::run(&conn, "select name, cast(d as varchar), v from t where id = 2", 10, None, None, &mut py).unwrap();
        assert_eq!(r["rows"][0], json!(["it's", "2026-05-31", null]));
        // a bad value saves nothing, not even the good one before it
        let bad = dbconn::apply(&conn, "main", "t", &[e("name", json!("zzz"), "VARCHAR"), e("d", json!("not a date"), "DATE")], None);
        assert!(bad.is_err());
        let r = dbconn::run(&conn, "select name from t where id = 2", 10, None, None, &mut py).unwrap();
        assert_eq!(r["rows"][0][0], json!("it's"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_uhnwi_report_reads_every_source_from_the_bank_tables() {
        if dbconn::demo_path().is_none() {
            return; // the demo database isn't built
        }
        let doc = uhnwi_fixed();
        let mut py = python::PyEngine::default();
        let mut engine = query::Engine::default();
        for ds in &doc.datasets {
            let q = match &ds.source {
                model::Source::Db { query, .. } => query,
                s => panic!("{} is not on the database: {s:?}", ds.id),
            };
            assert!(!q.contains("rpt."), "{} reads a report table", ds.id);
            let t = engine.table_all(&doc, ds, None, &mut py).unwrap_or_else(|e| panic!("{}: {e}", ds.id));
            assert!(!t.rows.is_empty(), "{} has no rows for the test client", ds.id);
        }
        // the period's return: net_twr_ltd at its end / the day before it starts − 1, for the client's accounts summed
        let port = doc.datasets.iter().find(|d| d.id == "port").unwrap();
        let t = engine.table_all(&doc, port, None, &mut py).unwrap();
        let ret = t.rows[0][t.col("ret").unwrap()].num().unwrap();
        assert!(ret > -0.5 && ret < 0.5, "return {ret}");
        let am = doc.datasets.iter().find(|d| d.id == "am").unwrap();
        let t = engine.table_all(&doc, am, None, &mut py).unwrap();
        let (ac, cc) = (t.col("acct").unwrap(), t.col("cum").unwrap());
        let last = t.rows.iter().filter(|r| r[ac].text() == "ALL").last().unwrap()[cc].num().unwrap();
        assert!((last - ret).abs() < 1e-9, "the monthly series ends at the period's return");
        // the largest operations' tables show their N largest rows (by size, withdrawals too), not all of them
        let res = engine.results(&doc, None, &mut py);
        let mut seen = 0;
        for w in doc.pages.iter().flat_map(|p| &p.widgets).filter(|w| w.binding.as_ref().is_some_and(|b| b.dataset == "ops" && b.limit > 0)) {
            let n = w.binding.as_ref().unwrap().limit as usize;
            let rows = res[&w.id]["rows"].as_array().unwrap();
            assert!(!rows.is_empty() && rows.len() <= n, "{}: {} rows", w.title, rows.len());
            let sizes: Vec<f64> = rows.iter().map(|r| r[3].as_f64().unwrap_or(0.0).abs()).collect();
            assert!(sizes.windows(2).all(|p| p[0] >= p[1]), "{}: not largest first {sizes:?}", w.title);
            seen += 1;
        }
        assert_eq!(seen, 2, "the largest operations and the taxes tables");
        // a position's price change is against its purchase price, not the period's start
        let hold = doc.datasets.iter().find(|d| d.id == "hold").unwrap();
        let t = engine.table_all(&doc, hold, None, &mut py).unwrap();
        let (a, p, c) = (t.col("avg").unwrap(), t.col("price").unwrap(), t.col("chg").unwrap());
        let mut n = 0;
        for r in &t.rows {
            if let (Some(a), Some(p), Some(c)) = (r[a].num(), r[p].num(), r[c].num()) {
                assert!((p / a - 1.0 - c).abs() < 1e-9, "chg {c} for price {p}, purchase {a}");
                n += 1;
            }
        }
        assert!(n > 0, "positions with a purchase price");
        // "Получено": dividends, coupons and maturities, per brokerage position, as the bank's operations say
        // (sold and matured positions stay in the table, so none of it is lost)
        let client = doc.params.values["client"].clone();
        let (s0, e0) = (doc.params.start.unwrap(), doc.params.date);
        let conn = model::Connection { id: "d".into(), name: "d".into(), kind: "duckdb".into(), path: dbconn::demo_path().unwrap(), url: String::new() };
        let sql = format!("select coalesce(sum(amount_rub), 0)::double from operations where op_type in ('Купон', 'Дивиденд', 'Погашение') and op_dt between '{s0}' and '{e0}'
                           and acct_cd in (select a.acct_cd from accounts a join clients c on c.id = a.client_id where c.name = '{client}' and a.src_cd = 'BROK')");
        let ops = dbconn::run(&conn, &sql, 5, None, None, &mut py).unwrap()["rows"][0][0].as_f64().unwrap_or(0.0);
        let hold = doc.datasets.iter().find(|d| d.id == "hold").unwrap();
        let t = engine.table_all(&doc, hold, None, &mut py).unwrap();
        let dc = t.col("divs").unwrap();
        let received: f64 = t.rows.iter().filter_map(|r| r[dc].num()).sum();
        assert!(received > 0.0 && (received - ops).abs() < 1.0, "received {received} vs operations {ops}");
        // the products add up to the portfolio's result (taxes, fees and cash revaluation included)
        let cat = doc.datasets.iter().find(|d| d.id == "cat").unwrap();
        let t = engine.table_all(&doc, cat, None, &mut py).unwrap();
        let ic = t.col("income").unwrap();
        let products: f64 = t.rows.iter().filter_map(|r| r[ic].num()).sum();
        let t2 = engine.table_all(&doc, port, None, &mut py).unwrap();
        let headline = t2.rows[0][t2.col("income").unwrap()].num().unwrap();
        assert!((products - headline).abs() < 1.0, "products {products} vs headline {headline}");
        // each account's position tables add up to its result (its taxes, fees and cash revaluation as a row)
        let accts = doc.datasets.iter().find(|d| d.id == "accounts").unwrap();
        let ta = engine.table_all(&doc, accts, None, &mut py).unwrap();
        let (kc, nc) = (ta.col("acct_cd").unwrap(), ta.col("kind").unwrap());
        for src in ["hold", "duh"] {
            let ds = doc.datasets.iter().find(|d| d.id == src).unwrap();
            let t = engine.table_all(&doc, ds, None, &mut py).unwrap();
            let (ac, ic) = (t.col("acct").unwrap(), t.col("income").unwrap());
            for r in ta.rows.iter().filter(|r| r[nc].text() == if src == "hold" { "BROK" } else { "DU" }) {
                let acct = r[kc].text();
                let table: f64 = t.rows.iter().filter(|x| x[ac].text() == acct).filter_map(|x| x[ic].num()).sum();
                let res = ta.rows.iter().find(|x| x[kc].text() == acct).and_then(|x| ta.col("result_txt").map(|c| x[c].text())).unwrap_or_default();
                let res: f64 = res.replace(['\u{a0}', ' ', '₽', '+'], "").replace('−', "-").parse().unwrap_or(f64::NAN);
                assert!((table - res).abs() <= 1.0, "{src} {acct}: table {table} vs account {res}");
            }
        }
        // the capital bridge adds up: start + money in + money out + income = the value at the end
        let bridge = doc.datasets.iter().find(|d| d.id == "bridge").unwrap();
        let t = engine.table_all(&doc, bridge, None, &mut py).unwrap();
        let ac = t.col("amount").unwrap();
        let sum: f64 = t.rows.iter().filter_map(|r| r[ac].num()).sum();
        let t2 = engine.table_all(&doc, port, None, &mut py).unwrap();
        let end = t2.rows[0][t2.col("value").unwrap()].num().unwrap();
        assert!((sum - end).abs() < 1.0, "bridge {sum} vs value {end}");
        // trade totals keep their precision (a donut's centre adds them up)
        let flows = doc.datasets.iter().find(|d| d.id == "flows").unwrap();
        let t = engine.table_all(&doc, flows, None, &mut py).unwrap();
        let ac = t.col("amount").unwrap();
        assert!(t.rows.iter().any(|r| r[ac].num().is_some_and(|v| (v * 10.0 - (v * 10.0).round()).abs() > 1e-6)), "flows amounts are rounded");
    }

    #[test]
    fn every_source_runs_for_every_client() {
        if dbconn::demo_path().is_none() {
            return; // the demo database isn't built
        }
        let mut doc = uhnwi_sample::template();
        doc.params.start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1);
        doc.params.date = chrono::NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let mut py = python::PyEngine::default();
        let mut engine = query::Engine::default();
        let list = doc.datasets.iter().find(|d| d.id == "client_list").unwrap().clone();
        let t = engine.table_all(&doc, &list, None, &mut py).unwrap();
        let cc = t.col("client").unwrap();
        let clients: Vec<String> = t.rows.iter().map(|r| r[cc].text()).collect();
        assert!(clients.len() > 30);
        let mut errors = Vec::new();
        for c in &clients {
            doc.params.values.insert("client".into(), c.clone());
            engine.clear();
            for ds in &doc.datasets {
                if let Err(e) = engine.table_all(&doc, ds, None, &mut py) {
                    errors.push(format!("{c} · {}: {}", ds.id, e.lines().next().unwrap_or("")));
                }
            }
        }
        assert!(errors.is_empty(), "{} failures:\n{}", errors.len(), errors.iter().take(15).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn a_database_connection_lists_tables_runs_sql_and_feeds_a_source() {
        let Some(path) = dbconn::demo_path() else { return }; // the demo database isn't built
        let conn = model::Connection { id: "c1".into(), name: "Demo".into(), kind: "duckdb".into(), path, url: String::new() };
        let mut py = python::PyEngine::default();
        // tables, with columns and row counts
        let t = dbconn::tables(&conn, None, &mut py).unwrap();
        let clients = t.as_array().unwrap().iter().find(|x| x["name"] == "clients").expect("clients table");
        assert_eq!(clients["rows"], json!(41), "40 clients and the test client");
        assert!(clients["columns"].as_array().unwrap().iter().any(|c| c["name"] == "sales_id"));
        // a CTE, a join and a window function, previewed
        let sql = "with last as (select acct_cd, max(report_dt) d from portf_acct group by 1)
                   select a.client_id, p.acct_cd, p.bal_out, rank() over (order by p.bal_out desc) as rnk
                   from portf_acct p join last l on l.acct_cd = p.acct_cd and l.d = p.report_dt join accounts a on a.acct_cd = p.acct_cd";
        let r = dbconn::run(&conn, sql, 5, None, None, &mut py).unwrap();
        assert_eq!(r["rows"].as_array().unwrap().len(), 5);
        assert!(r["more"].as_bool().unwrap() && r["total"].as_u64().unwrap() > 100);
        assert_eq!(r["columns"][3]["name"], "rnk");
        // nothing that writes
        assert!(dbconn::run(&conn, "delete from clients", 5, None, None, &mut py).is_err());
        // a data source on the connection, with the report's period filled in
        let mut doc = sample::template();
        doc.connections.push(conn);
        let ds = Dataset {
            id: "dsdb".into(),
            name: "Account returns".into(),
            source: model::Source::Db { connection: "c1".into(), query: "select report_dt, acct_cd, net_twr from portf_acct where report_dt between :start and :end".into() },
            date_field: Some("report_dt".into()),
            ..Default::default()
        };
        doc.params.date = chrono::NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let mut engine = query::Engine::default();
        let rows = engine.table_all(&doc, &ds, None, &mut py).unwrap();
        assert!(!rows.rows.is_empty());
        let dc = rows.col("report_dt").unwrap();
        // :end is the end of the window the report needs (the period's month)
        let end = chrono::NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();
        assert!(rows.rows.iter().all(|r| r[dc].date().is_some_and(|d| d <= end)), "rows after :end");
        // a removed connection says so
        doc.connections.clear();
        let mut engine = query::Engine::default();
        assert!(engine.table_all(&doc, &ds, None, &mut py).unwrap_err().contains("connection"));
    }

    #[test]
    fn custom_variables_fill_texts() {
        let mut doc = sample::template();
        doc.variables = vec![
            model::Variable { name: "asof".into(), kind: "text".into(), value: "as of {{end}}".into(), ..Default::default() },
            model::Variable { name: "north".into(), kind: "sql".into(), value: "select sum(revenue) from sales where region = 'North' and date between :start and :end".into(), format: "number".into(), ..Default::default() },
            model::Variable { name: "share".into(), kind: "sql".into(), value: "select 0.256".into(), format: "percent".into(), ..Default::default() },
        ];
        let mut app = app_with(doc);
        let r = app.resp();
        let vars = r["variables"].as_array().unwrap();
        let val = |t: &str| vars.iter().find(|v| v["token"] == json!(t)).map(|v| v["value"].as_str().unwrap_or("").to_string()).unwrap();
        let end = val("{{end}}");
        assert_eq!(val("{{asof}}"), format!("as of {end}"), "text variables expand what they hold");
        let north = val("{{north}}");
        assert!(north.chars().any(|c| c.is_ascii_digit()) && !north.contains("{{"), "sql variable resolves: {north}");
        assert_eq!(val("{{share}}"), "25.6%");
        assert!(vars.iter().any(|v| v["token"] == json!("{{region}}")), "parameters are listed");
    }

    #[test]
    fn report_users_change_only_offered_filters() {
        let mut doc = sample::template();
        doc.policy.period = "show".into();
        doc.param_defs[0].user = "show".into();
        to_report(&mut doc, None);
        let mut app = app_with(doc);
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.grain = "year".into();
        assert!(apply(&mut app, Edit::SetParams { params: p }).is_err(), "the period is fixed");
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.values.insert("region".into(), "North".into());
        assert!(apply(&mut app, Edit::SetParams { params: p }).is_err(), "the region is fixed");
        let mut p = app.doc.as_ref().unwrap().params.clone();
        p.compare = "year".into();
        assert!(apply(&mut app, Edit::SetParams { params: p }).is_ok(), "the comparison is still offered");
    }

    #[test]
    fn an_account_filter_reaches_every_source() {
        let inline = |id: &str, name: &str, cols: &[&str], rows: Vec<Vec<Json>>| Dataset {
            id: id.into(),
            name: name.into(),
            source: model::Source::Inline { columns: cols.iter().map(|c| c.to_string()).collect(), rows },
            ..Default::default()
        };
        let mut doc = Doc::default();
        doc.datasets = vec![
            inline("h", "Holdings", &["account", "value"], vec![vec![json!("A1"), json!(100)], vec![json!("A2"), json!(50)]]),
            inline("t", "Trades", &["acct", "amount"], vec![vec![json!("A1"), json!(7)], vec![json!("A2"), json!(3)], vec![json!("A2"), json!(4)]]),
            inline("fx", "FX", &["ccy", "rate"], vec![vec![json!("USD"), json!(90)]]),
        ];
        doc.datasets.push(Dataset {
            id: "q".into(),
            name: "Joined".into(),
            source: model::Source::Query { sql: "select h.account, sum(h.value) as value, (select sum(amount) from trades) as traded from holdings h group by h.account".into() },
            ..Default::default()
        });
        // a window total over the source's rows (a level-of-detail field)
        doc.datasets[0].calc_fields.push(model::CalcField { name: "all_value".into(), expr: "sum(value) over ()".into() });
        let mut columns = std::collections::BTreeMap::new();
        columns.insert("t".to_string(), "acct".to_string());
        doc.param_defs = vec![model::ParamDef {
            name: "account".into(), label: "Account".into(), kind: "list".into(), default: "All".into(), field: "account".into(), columns,
            source: Some(model::ParamSource { dataset: "h".into(), field: "account".into(), label: String::new() }), ..Default::default()
        }];
        doc.params.values.insert("account".into(), "A1".into());
        let mut app = app_with(doc);
        let doc = app.doc.clone().unwrap();
        let rows = |app: &mut App, id: &str| { let ds = doc.dataset(id).unwrap().clone(); app.engine.table(&doc, &ds, None, &mut app.py).unwrap() };
        assert_eq!(rows(&mut app, "h").rows.len(), 1, "holdings by its account column");
        assert_eq!(rows(&mut app, "t").rows.len(), 1, "trades by acct, the column chosen for it");
        assert_eq!(rows(&mut app, "fx").rows.len(), 1, "a source without the column stays whole");
        let h = rows(&mut app, "h");
        assert_eq!(h.rows[0][h.col("all_value").unwrap()].num(), Some(100.0), "window totals see only the account");
        let q = rows(&mut app, "q");
        assert_eq!(q.rows.len(), 1);
        assert_eq!(q.rows[0][q.col("traded").unwrap()].num(), Some(7.0), "SQL over sources sees them filtered");
        assert_eq!(app.engine.sql_lookup(&doc, "select sum(amount) from trades", None, &mut app.py), "7", "{{sql:}} values too");
        // the choices stay complete
        let opts = app.engine.param_options(&doc, None, &mut app.py);
        assert_eq!(opts["account"], json!(["A1", "A2"]));
    }

    #[test]
    fn a_report_is_offered_the_themes_that_have_its_looks() {
        let doc = uhnwi_sample::template();
        let list = theme_list(None, Some(&doc));
        let fits = |id: &str| list.as_array().unwrap().iter().find(|t| t["id"] == id).unwrap()["fits"].as_bool().unwrap();
        for id in ["builtin:quire", "builtin:alpine", "builtin:green", "builtin:graphite"] {
            assert!(fits(id), "{id} has every UHNWI look");
        }
    }

    #[test]
    fn theme_library_rename_duplicate_delete() {
        let dir = std::env::temp_dir().join(format!("quire_lib_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let id = |v: Json| v.as_str().unwrap().to_string();
        let copy = id(library_op(&dir, "duplicate", "builtin:alpine", "", None).unwrap());
        assert!(copy.ends_with("Alpine copy.qtheme"));
        let again = id(library_op(&dir, "duplicate", "builtin:alpine", "", None).unwrap());
        assert!(again.ends_with("Alpine copy 2.qtheme"));
        let renamed = id(library_op(&dir, "rename", &copy, "Family office", None).unwrap());
        assert!(renamed.ends_with("Family office.qtheme"));
        assert!(!Path::new(copy.strip_prefix("file:").unwrap()).exists());
        assert_eq!(load_theme(&renamed).unwrap().0.name, "Family office");
        // the theme open in the studio and built-ins are protected
        let open = PathBuf::from(renamed.strip_prefix("file:").unwrap());
        assert!(library_op(&dir, "delete", &renamed, "", Some(&open)).is_err());
        assert!(library_op(&dir, "delete", "builtin:graphite", "", None).is_err());
        assert!(library_op(&dir, "delete", "file:/etc/hosts", "", None).is_err());
        library_op(&dir, "delete", &renamed, "", None).unwrap();
        library_op(&dir, "delete", &again, "", None).unwrap();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir(&dir);
    }

    pub(super) fn app_with(doc: Doc) -> App {
        let mut a = App {
            doc: None,
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            engine: Engine::default(),
            py: PyEngine::new(),
            user: "Tester".into(),
            notice: None,
            parked: Vec::new(),
            active_id: 0,
            active_link: None,
            next_id: 0,
            studio_base: None,
            settings: Settings::default(),
            session: None,
            clients: None,
            store_path: None,
            data_params: Params::default(),
            data_report: None,
        work: Default::default(),
        };
        a.load(doc, None);
        a
    }

    #[test]
    fn a_composite_lays_out_its_parts_with_their_own_data() {
        let d = sample::template();
        let page = d.pages[0].id.clone();
        let ds = d.datasets[0].id.clone();
        let mut app = app_with(d);
        // a composite reading one source: its card holds a figure and a mini chart
        let comp = Widget { kind: "composite".into(), title: "Sales at a glance".into(), w: 6, h: 4,
            binding: Some(model::Binding { dataset: ds.clone(), ..Default::default() }), opts: json!({"grid": {"cols": 12, "rows": 6}}), ..Default::default() };
        run_edit(&mut app, Edit::AddWidget { page, widget: comp, at: None }).unwrap();
        let cid = app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.kind == "composite").unwrap().id.clone();
        let rev = model::Measure { field: "revenue".into(), agg: "sum".into(), label: "Revenue".into(), format: "currency".into(), ..Default::default() };
        let kpi = Widget { kind: "kpi".into(), w: 4, h: 6, binding: Some(model::Binding { measures: vec![rev.clone()], ..Default::default() }), style: json!({"card": false}), ..Default::default() };
        let bar = Widget { kind: "bar".into(), w: 8, h: 6, binding: Some(model::Binding { dims: vec!["region".into()], measures: vec![rev], ..Default::default() }), style: json!({"card": false}), ..Default::default() };
        run_edit(&mut app, Edit::AddPart { id: cid.clone(), part: kpi }).unwrap();
        run_edit(&mut app, Edit::AddPart { id: cid.clone(), part: bar }).unwrap();
        let w = app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.id == cid).unwrap().clone();
        assert_eq!(w.parts.iter().map(|p| (p.id.as_str(), p.x, p.w)).collect::<Vec<_>>(), [("p1", 0, 4), ("p2", 4, 8)], "placed side by side");
        let r = app.resp();
        let vm = r["pages"][0]["widgets"].as_array().unwrap().iter().find(|v| v["id"] == json!(cid)).unwrap().clone();
        let parts = vm["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        let body = &vm["body"];
        for p in parts {
            assert!(p["data"].get("error").is_none() && !p["data"].is_null(), "{}", p["data"]);
            let f = &p["frame"];
            assert!(f["x"].as_f64() >= body["x"].as_f64() && f["x"].as_f64().unwrap() + f["w"].as_f64().unwrap() <= body["x"].as_f64().unwrap() + body["w"].as_f64().unwrap() + 0.01, "inside the card: {f} in {body}");
        }
        assert_eq!(parts[1]["data"]["rows"].as_array().unwrap().len() > 1, true, "the chart reads the shared source by region");
        // a part is changed, moved and removed by its full id; one undo step each
        let pid = format!("{cid}::p2");
        run_edit(&mut app, Edit::UpdateWidget { id: pid.clone(), patch: json!({"title": "By region", "opts": {"viz": {"labels": "none"}}}) }).unwrap();
        run_edit(&mut app, Edit::SetRect { id: pid.clone(), rect: layout::Rect { x: 4, y: 0, w: 8, h: 3 } }).unwrap();
        let p2 = app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.id == cid).unwrap().parts[1].clone();
        assert_eq!((p2.id.as_str(), p2.title.as_str(), p2.h), ("p2", "By region", 3));
        run_edit(&mut app, Edit::RemoveWidget { id: pid }).unwrap();
        assert_eq!(app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.id == cid).unwrap().parts.len(), 1);
        let prev = app.undo.pop().unwrap();
        app.doc = Some(prev);
        assert_eq!(app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.id == cid).unwrap().parts.len(), 2, "one undo brings it back");
        // a composite as a widget type: a widget made from it has its parts; a change to the type reaches it
        let comp = app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.id == cid).unwrap().clone();
        let mut wt = model::WidgetType { id: "glance".into(), name: "At a glance".into(), kind: "composite".into(), binding: comp.binding.clone(), w: 6, h: 4, version: 1, parts: comp.parts.clone(), ..Default::default() };
        let page = app.doc.as_ref().unwrap().pages[0].id.clone();
        run_edit(&mut app, Edit::AddTypedWidget { page, wt: wt.clone(), at: None }).unwrap();
        let made = || app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.wtype == "glance").unwrap().clone();
        assert_eq!(made().parts.len(), 2);
        wt.parts.truncate(1);
        wt.version = 2;
        run_edit(&mut app, Edit::SetWidgetType { wt }).unwrap();
        let made = app.doc.as_ref().unwrap().pages[0].widgets.iter().find(|w| w.wtype == "glance").unwrap().clone();
        assert_eq!(made.parts.len(), 1, "the type's new parts reach its widgets");
    }

    #[test]
    fn sample_template_fits_and_queries() {
        let mut app = app_with(sample::template());
        let r = app.resp();
        for p in r["pages"].as_array().unwrap() {
            for w in p["widgets"].as_array().unwrap() {
                if let Some(d) = w.get("data").filter(|d| !d.is_null()) {
                    assert!(d.get("error").is_none(), "widget {} failed: {}", w["id"], d);
                }
            }
            let errors: Vec<&Json> = p["issues"].as_array().unwrap().iter().filter(|i| i["severity"] != "info").collect();
            assert!(errors.is_empty(), "page {} has issues: {:?}", p["id"], errors);
        }
    }

    #[test]
    fn new_dataset_with_old_dates_still_shows_data() {
        // the reported bug: a fresh template, a dataset from 2023, a table → empty
        let mut app = app_with(sample::blank());
        let rows: Vec<Vec<Json>> = vec![
            vec![json!("2023-03-02"), json!("BUY"), json!("$1,200")],
            vec![json!("2023-03-09"), json!("SELL"), json!("$800")],
            vec![json!("2023-02-11"), json!("BUY"), json!("$300")],
        ];
        let ds = Dataset {
            name: "Trades".into(),
            source: model::Source::Inline { columns: vec!["date".into(), "side".into(), "amount".into()], rows },
            date_field: Some("date".into()),
            ..Default::default()
        };
        let op: Edit = serde_json::from_value(json!({"op": "upsert_dataset", "dataset": ds})).unwrap();
        apply(&mut app, op).unwrap();
        app.snap_period("");
        let doc = app.doc.as_ref().unwrap();
        assert_eq!(doc.params.date, period::ymd(2023, 3, 9));
        let ds_id = doc.datasets[0].id.clone();
        let page = doc.pages[0].id.clone();
        let w: Widget = serde_json::from_value(json!({
            "kind": "table", "w": 6, "h": 4,
            "binding": {"dataset": ds_id, "dims": ["side"], "measures": [{"field": "amount", "agg": "sum"}]}
        }))
        .unwrap();
        apply(&mut app, Edit::AddWidget { page, widget: w, at: None }).unwrap();
        let r = app.resp();
        let t = r["pages"][0]["widgets"].as_array().unwrap().iter().find(|w| w["data"]["rows"].is_array()).unwrap();
        assert_eq!(t["data"]["rows"], json!([["BUY", 1200.0], ["SELL", 800.0]]));
        assert!(r["notice"].as_str().unwrap().contains("March 2023"));
    }

    #[test]
    fn column_styles_follow_measures_when_reordered() {
        let mut app = app_with(sample::template());
        let doc = app.doc.as_ref().unwrap();
        let t = doc.pages[1].widgets.iter().find(|w| w.kind == "table").unwrap();
        let ids: Vec<String> = t.binding.as_ref().unwrap().measures.iter().map(|m| m.id.clone()).collect();
        assert_eq!(ids, vec!["m0", "m1", "m2", "m3", "m4"], "old files keep their keys");
        let mut ms = t.binding.as_ref().unwrap().measures.clone();
        ms.swap(0, 1);
        let id = t.id.clone();
        apply(&mut app, Edit::UpdateWidget { id: id.clone(), patch: json!({"binding": {"measures": ms}}) }).unwrap();
        let r = app.resp();
        let w = r["pages"][1]["widgets"].as_array().unwrap().iter().find(|w| w["id"] == json!(id)).unwrap();
        let keys: Vec<&str> = w["data"]["columns"].as_array().unwrap().iter().map(|c| c["key"].as_str().unwrap()).collect();
        assert_eq!(&keys[1..3], &["m1", "m0"]);
    }

    #[test]
    fn long_tables_continue_on_new_pages() {
        let mut app = app_with(sample::template());
        let page = app.doc.as_ref().unwrap().pages[0].id.clone();
        let w: Widget = serde_json::from_value(json!({
            "kind": "table", "title": "Order lines", "w": 12, "h": 3,
            "binding": {"dataset": "sales", "mode": "rows", "columns": ["date", "region", "product", "revenue"], "window": "period"},
            "opts": {"totals": true}
        }))
        .unwrap();
        let mut d = app.doc.clone().unwrap();
        d.pages.push(Page { id: "pX".into(), title: "Orders".into(), widgets: vec![], auto: false, section: String::new(), plain: false, ..Default::default() });
        app.doc = Some(d);
        let _ = page;
        apply(&mut app, Edit::AddWidget { page: "pX".into(), widget: w, at: None }).unwrap();
        app.arrange(2, None, None, false);
        app.sync_chains();
        let doc = app.doc.clone().unwrap();
        let autos: Vec<&Page> = doc.pages.iter().filter(|p| p.auto).collect();
        assert!(autos.len() >= 2, "60 order lines need continuation pages, got {}", autos.len());
        let r = app.resp();
        let pieces: Vec<&Json> = r["pages"].as_array().unwrap().iter().flat_map(|p| p["widgets"].as_array().unwrap()).filter(|w| w["table"].is_object() && w["data"]["detail"] == json!(true)).collect();
        let shown: u64 = pieces.iter().map(|w| w["table"]["visible"].as_u64().unwrap()).sum();
        assert_eq!(shown, 60, "every row appears exactly once");
        assert_eq!(pieces.last().unwrap()["table"]["totals"], json!(true));
        assert_eq!(pieces[0]["table"]["totals"], json!(false));
        // syncing again is stable: no extra pages
        let before = app.doc.as_ref().unwrap().pages.len();
        app.sync_chains();
        app.sync_chains();
        assert_eq!(app.doc.as_ref().unwrap().pages.len(), before);
        // turning pagination off removes the pieces and their pages
        let id = doc.pages[2].widgets[0].id.clone();
        apply(&mut app, Edit::UpdateWidget { id, patch: json!({"opts": {"table": {"paginate": false}}}) }).unwrap();
        app.sync_chains();
        assert!(app.doc.as_ref().unwrap().pages.iter().all(|p| !p.auto));
    }

    #[test]
    fn each_continuation_page_follows_its_own_table() {
        if dbconn::demo_path().is_none() {
            return; // the UHNWI report reads the demo database
        }
        // the broker's and the DU's holdings both continue: the broker's new page must not push the DU's before its table
        let mut app = app_with(uhnwi_fixed());
        app.sync_chains();
        let doc = app.doc.clone().unwrap();
        let mut owner: Option<&Page> = None;
        let mut autos = 0;
        for p in &doc.pages {
            if !p.auto {
                owner = Some(p);
                continue;
            }
            autos += 1;
            let src = p.widgets.iter().find_map(|w| vm::continues(w)).expect("a continuation piece");
            // right after the table, or after a designed page that continues it
            assert!(owner.is_some_and(|o| o.widgets.iter().any(|w| w.id == src || vm::continues(w) == Some(src))), "“{}” is not after its table", p.title);
        }
        assert!(autos >= 1);
    }

    #[test]
    fn sql_fields_transforms_joins_and_filters() {
        let mut app = app_with(sample::template());
        // a calculated field on the sample sales data
        let mut sales = app.doc.as_ref().unwrap().datasets[0].clone();
        sales.calc_fields = vec![model::CalcField { name: "margin_pct".into(), expr: "gross_profit / revenue".into() }];
        run_edit(&mut app, serde_json::from_value(json!({"op": "upsert_dataset", "dataset": sales})).unwrap()).unwrap();
        let fields = &app.doc.as_ref().unwrap().datasets[0].fields;
        assert!(fields.iter().any(|f| f.name == "margin_pct" && f.kind == "number"), "{fields:?}");
        // a second dataset joined to it in SQL
        let targets = Dataset {
            name: "Targets".into(),
            source: model::Source::Inline { columns: vec!["region".into(), "target".into()], rows: vec![vec![json!("North"), json!(1)], vec![json!("South"), json!(2)]] },
            transform: "select t.region, t.target, count(*) as lines from targets t join sales s on s.region = t.region group by all order by t.region".into(),
            ..Default::default()
        };
        // a transform can't mention itself without a self-join name; use a fresh name
        let mut joined = targets.clone();
        joined.name = "Joined".into();
        joined.transform = joined.transform.replace("from targets t", "from data t");
        run_edit(&mut app, serde_json::from_value(json!({"op": "upsert_dataset", "dataset": joined})).unwrap()).unwrap();
        let doc = app.doc.clone().unwrap();
        let ds = doc.datasets.last().unwrap();
        let t = app.engine.table(&doc, ds, None, &mut app.py).unwrap();
        assert_eq!(t.columns, vec!["region", "target", "lines"]);
        assert_eq!(t.rows.len(), 2);
        assert!(t.rows[0][2].num().unwrap() > 100.0, "joined with the sales rows");
        // a widget-level SQL filter
        let page = doc.pages[0].id.clone();
        let w: Widget = serde_json::from_value(json!({
            "kind": "table", "w": 6, "h": 4,
            "binding": {"dataset": "sales", "dims": ["region"], "window": "all", "where_sql": "region in ('North', 'East') and margin_pct > 0.2",
                "measures": [{"field": "revenue", "agg": "sum"}]}
        }))
        .unwrap();
        run_edit(&mut app, Edit::AddWidget { page, widget: w, at: Some(Rect { x: 0, y: 12, w: 6, h: 4 }) }).unwrap();
        let r = app.resp();
        let tw = r["pages"][0]["widgets"].as_array().unwrap().last().unwrap();
        let regions: Vec<&str> = tw["data"]["rows"].as_array().unwrap().iter().map(|r| r[0].as_str().unwrap()).collect();
        assert_eq!(regions.len(), 2, "{}", tw["data"]);
        assert!(regions.contains(&"North") && regions.contains(&"East"));
        // and a broken one explains itself
        let id = tw["id"].as_str().unwrap().to_string();
        run_edit(&mut app, Edit::UpdateWidget { id, patch: json!({"binding": {"where_sql": "no_such_column > 1"}}) }).unwrap();
        let r = app.resp();
        let tw = r["pages"].as_array().unwrap().iter().flat_map(|p| p["widgets"].as_array().unwrap()).find(|w| w["data"]["error"].is_string()).expect("error shown");
        assert!(tw["data"]["error"].as_str().unwrap().starts_with("SQL filter"));
    }

    #[test]
    fn header_footer_bands_with_data_and_sql() {
        // an older file: footer as a switch, tabs as a switch, footer texts as strings
        let old = json!({"page": {"footer": true, "header_tabs": true, "footer_left": "{{title}}", "footer_right": "{{page}}"}, "pages": [{"id": "p1", "title": "A", "widgets": []}]});
        let mut d: Doc = serde_json::from_value(old).unwrap();
        d.normalize();
        assert!(d.page.footer.show && d.page.header.show);
        assert_eq!(d.page.footer.left.text, "{{title}}");
        assert_eq!(d.page.header.tabs, "left");
        // data and SQL in the footer
        let mut doc = sample::template();
        doc.page.footer.center.text = "Revenue {{sql: select sum(revenue) from sales where region = 'North'}} · rows {{data:Sales.region|count}}".into();
        let mut app = app_with(doc);
        let r = app.resp();
        let c = r["footer"]["center"]["text"].as_str().unwrap().to_string();
        assert!(c.starts_with("Revenue ") && !c.contains("{{") && !c.contains('['), "{c}");
        assert!(c.contains("rows "), "{c}");
        assert_eq!(r["footer"]["left"]["font"]["family"], json!("Inter"));
    }

    #[test]
    fn report_users_are_limited() {
        let mut doc = sample::template();
        to_report(&mut doc, None);
        let heading = doc.pages[0].widgets[0].id.clone();
        let chart = doc.pages[0].widgets.iter().find(|w| w.kind == "line").unwrap().id.clone();
        let mut app = app_with(doc);
        assert!(apply(&mut app, Edit::RemoveWidget { id: heading }).is_err(), "locked heading");
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"binding": {"dataset": "x"}}) }).is_err());
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"binding": {"window": "trailing6"}, "style": {"color": "#e34948"}}) }).is_ok());
        assert!(apply(&mut app, Edit::UpsertDataset { dataset: Dataset::default() }).is_err());
        // another prepared look, the same data as another chart, a hidden title and smaller text
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"variant": "x"}) }).is_ok(), "look");
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"kind": "bar", "opts": {"orientation": "vertical"}}) }).is_ok(), "chart type");
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"style": {"hideTitle": true, "textScale": 0.85, "pad": 4}}) }).is_ok(), "fit");
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"kind": "text"}) }).is_err(), "a chart stays a chart");
        // without "Change look" none of it
        app.doc.as_mut().unwrap().policy.restyle = false;
        assert!(apply(&mut app, Edit::UpdateWidget { id: chart.clone(), patch: json!({"variant": ""}) }).is_err());
    }

    #[test]
    fn a_hidden_title_is_kept_but_not_drawn() {
        let doc = sample::template();
        let mut w = doc.pages[0].widgets.iter().find(|w| w.kind == "line").unwrap().clone();
        let frame = model::Frame { x: 0.0, y: 0.0, w: 300.0, h: 200.0 };
        let shown = vm::widget_vm(&doc, &w, frame.clone(), None);
        w.style = json!({"hideTitle": true});
        let hidden = vm::widget_vm(&doc, &w, frame, None);
        assert!(shown.title.is_some() && hidden.title.is_none());
        assert!(!w.title.is_empty(), "the title text stays");
    }

    #[test]
    fn adding_a_comment_keeps_the_page_fitting() {
        let mut doc = sample::template();
        to_report(&mut doc, None);
        let page = doc.pages[0].id.clone();
        let mut app = app_with(doc);
        let long = "A long commentary paragraph that wraps over several lines to make the comment taller than its default size. ".repeat(4);
        let w = Widget { kind: "comment".into(), w: 6, h: 2, opts: json!({"text": long}), ..Default::default() };
        let touched = apply(&mut app, Edit::AddWidget { page: page.clone(), widget: w, at: None }).unwrap();
        for t in touched {
            app.arrange(t.page, t.pinned.as_deref(), t.opts, t.firm);
        }
        let r = app.resp();
        let p = &r["pages"][0];
        let overflow = p["issues"].as_array().unwrap().iter().any(|i| i["kind"] == "overflow" || i["kind"] == "text_clipped");
        assert!(!overflow, "auto-arrange made room: {}", p["issues"]);
        let doc = app.doc.as_ref().unwrap();
        let bottom = doc.pages[0].widgets.iter().map(|w| w.y + w.h).max().unwrap();
        assert!(bottom <= doc.page.rows);
    }
}

#[cfg(test)]
mod dump {
    use super::*;

    /// Writes UI state snapshots for visual checks: `QUIRE_DUMP=dir cargo test dump -- --ignored`
    #[test]
    #[ignore]
    fn dump_states() {
        let dir = std::env::var("QUIRE_DUMP").expect("QUIRE_DUMP");
        let mut a = tests_app(sample::template());
        std::fs::write(format!("{dir}/template.json"), a.resp().to_string()).unwrap();
        let mut a = tests_app(client_sample::template());
        std::fs::write(format!("{dir}/client.json"), a.resp().to_string()).unwrap();
        let mut a = tests_app(uhnwi_fixed());
        std::fs::write(format!("{dir}/uhnwi.json"), a.resp().to_string()).unwrap();
        // the same report under every built-in UHNWI theme
        for (id, look) in uhnwi_theme::LOOKS {
            let mut doc = uhnwi_fixed();
            theme_studio::apply(&mut doc, &uhnwi_theme::theme_from(look), &[]);
            let mut a = tests_app(doc);
            std::fs::write(format!("{dir}/uhnwi_{id}.json"), a.resp().to_string()).unwrap();
        }
        let mut doc = sample::template();
        to_report(&mut doc, None);
        let mut a = tests_app(doc);
        std::fs::write(format!("{dir}/report.json"), a.resp().to_string()).unwrap();
    }

    fn tests_app(doc: Doc) -> App {
        let mut a = App {
            doc: None,
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            engine: Engine::default(),
            py: PyEngine::new(),
            user: "Tester".into(),
            notice: None,
            parked: Vec::new(),
            active_id: 0,
            active_link: None,
            next_id: 0,
            studio_base: None,
            settings: Settings::default(),
            session: None,
            clients: None,
            store_path: None,
            data_params: Params::default(),
            data_report: None,
        work: Default::default(),
        };
        a.load(doc, None);
        a
    }
}

#[cfg(test)]
mod export_check {
    /// Builds PDF/PPTX from payloads the UI produced: `QUIRE_DUMP=dir cargo test export_check -- --ignored`
    #[test]
    #[ignore]
    fn export_payloads() {
        let dir = std::env::var("QUIRE_DUMP").expect("QUIRE_DUMP");
        let pages: Vec<String> = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/pdf_pages.json")).unwrap()).unwrap();
        let pdf = crate::export::pdf(&pages, 841.89, 595.28, "Check").unwrap();
        std::fs::write(format!("{dir}/check.pdf"), pdf).unwrap();
        // one file per page too, for quick visual checks
        for (i, p) in pages.iter().enumerate() {
            let one = crate::export::pdf(std::slice::from_ref(p), 841.89, 595.28, "Check").unwrap();
            std::fs::write(format!("{dir}/check_{:02}.pdf", i + 1), one).unwrap();
        }
        let slides: Vec<crate::export::Slide> = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/pptx_pages.json")).unwrap()).unwrap();
        let pptx = crate::export::pptx(&slides, 841.89, 595.28, "Check").unwrap();
        std::fs::write(format!("{dir}/check.pptx"), pptx).unwrap();
    }
}

#[cfg(test)]
mod dev_helpers {
    //! Developer helpers run by hand (`--ignored`). The UI is driven in a headless browser through
    //! the web server instead (`cargo run --bin quire-server`, see dev/shots/README.md).
    use super::*;

    /// The session rebuild on a copy of a real log: `REBUILD_DIR=/tmp/x cargo test rebuild_on_copy -- --ignored --nocapture`
    /// (the folder holds activity.jsonl and ai_log.jsonl); prints the sessions made.
    #[test]
    #[ignore]
    fn rebuild_on_copy() {
        let dir = std::path::PathBuf::from(std::env::var("REBUILD_DIR").expect("REBUILD_DIR"));
        let n = audit::rebuild_once(&dir, Some(&dir));
        let again = audit::rebuild_once(&dir, Some(&dir));
        let all = audit::sessions(Some(&dir), Some(&dir), &audit::Filter::default());
        eprintln!("rebuilt {n}, again {again}");
        for s in all["sessions"].as_array().unwrap().iter().rev() {
            eprintln!("{} – {} · {} · {} · {} · client {} · edits {} · exports {} · AI {} · {}{}", &s["start"].as_str().unwrap()[..16], &s["end"].as_str().unwrap()[11..16], s["user"], s["kind"], s["title"], s["client"], s["edits"], s["exports"].as_array().unwrap().len(), s["ai"], s["reason"], if s["rebuilt"] == true { " · rebuilt" } else { "" });
        }
    }

    /// A brief's pages (SVG files, as `dev/shots/brief.mjs` writes them) as a PDF:
    /// `BRIEF_SVGS=/tmp/x cargo test brief_pdf_from_svgs -- --ignored` writes /tmp/x/brief.pdf.
    #[test]
    #[ignore]
    fn brief_pdf_from_svgs() {
        let dir = std::path::PathBuf::from(std::env::var("BRIEF_SVGS").expect("BRIEF_SVGS"));
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "svg")).collect();
        files.sort();
        let pages: Vec<String> = files.iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
        let pdf = export::pdf(&pages, 595.28, 841.89, "Brief").unwrap();
        std::fs::write(dir.join("brief.pdf"), pdf).unwrap();
    }

}

#[cfg(test)]
mod real_data {
    use super::*;

    /// End-to-end check on a real workbook: `QUIRE_XLSX=path cargo test real_data -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn assets_workbook() {
        let path = std::env::var("QUIRE_XLSX").expect("QUIRE_XLSX");
        let mut app = App {
            doc: None,
            path: None,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            engine: Engine::default(),
            py: PyEngine::new(),
            user: "Tester".into(),
            notice: None,
            parked: Vec::new(),
            active_id: 0,
            active_link: None,
            next_id: 0,
            studio_base: None,
            settings: Settings::default(),
            session: None,
            clients: None,
            store_path: None,
            data_params: Params::default(),
            data_report: None,
        work: Default::default(),
        };
        app.load(sample::blank(), None);
        let ds = Dataset {
            name: "Assets".into(),
            source: model::Source::Excel { path: path.clone(), sheet: String::new() },
            date_field: Some("report_dt".into()),
            ..Default::default()
        };
        let t = app.engine.table(app.doc.as_ref().unwrap(), &ds, None, &mut app.py).unwrap();
        println!("fields: {:?}", t.fields());
        println!("snapshot hint: {}", looks_like_snapshots(&t, Some("report_dt")));
        let mut ds = ds;
        ds.snapshot = true;
        let op: Edit = serde_json::from_value(json!({"op": "upsert_dataset", "dataset": ds})).unwrap();
        apply(&mut app, op).unwrap();
        app.refresh_fields();
        app.snap_period("");
        println!("notice: {:?}", app.notice);
        let doc = app.doc.as_ref().unwrap();
        let ds_id = doc.datasets[0].id.clone();
        let page = doc.pages[0].id.clone();
        for w in [
            json!({"kind": "kpi", "title": "AUM", "w": 3, "h": 2, "binding": {"dataset": ds_id, "measures": [{"field": "amount", "agg": "closing", "format": "compact"}]}}),
            json!({"kind": "table", "title": "By product group", "w": 6, "h": 4, "binding": {"dataset": ds_id, "dims": ["product_group", "product_name"], "measures": [
                {"field": "amount", "agg": "closing", "label": "AUM"},
                {"field": "amount", "agg": "closing", "calc": "pct", "label": "Δ%"},
                {"field": "amount", "agg": "sum", "label": "naive sum"}]}}),
        ] {
            let w: Widget = serde_json::from_value(w).unwrap();
            apply(&mut app, Edit::AddWidget { page: page.clone(), widget: w, at: None }).unwrap();
        }
        let r = app.resp();
        println!("period: {}", r["period"]["label"]);
        for w in r["pages"][0]["widgets"].as_array().unwrap() {
            if let Some(d) = w.get("data").filter(|d| !d.is_null()) {
                println!("{} → kpi={} rows={} total={}", w["id"], d["kpi"]["value"], d["rows"], d["total_row"]);
            }
        }
        // DuckDB over the same numbers via an in-memory table
        let sql = "select * from (values (DATE '2024-12-31', 'Banking', 10.5::DECIMAL(10,2)), (DATE '2024-12-30', 'Investment', 3)) t(report_dt, product_group, amount)";
        let mut ctx = data::LoadCtx { base_dir: None, range: None, values: &Default::default(), py: &mut app.py, sources: None };
        let dt = data::load(&model::Source::Duckdb { path: String::new(), query: sql.into() }, &mut ctx).unwrap();
        println!("duckdb: {:?} {:?}", dt.columns, dt.rows);
        assert_eq!(dt.rows[0][0], data::Val::Date(period::ymd(2024, 12, 31)));
        assert_eq!(dt.rows[0][2], data::Val::Num(10.5));
    }
}

/// The UHNWI template on the period its checks were written for (01.01–15.06.2026), not today's.
#[cfg(test)]
mod web_hardening {
    use super::*;

    /// The page sends a connection back with its password hidden: the kept one is put back; a new
    /// password the page typed is taken; a connection without a kept password loses the mask.
    #[test]
    fn hidden_passwords_are_put_back() {
        let st = auth::Store { connections: vec![model::Connection { id: "pg".into(), kind: "postgres".into(), url: "postgres://u:real@h/db".into(), ..Default::default() }], ..Default::default() };
        let sent = |url: &str, id: &str| model::Connection { id: id.into(), kind: "postgres".into(), url: url.into(), ..Default::default() };
        assert_eq!(real_conn_in(&st, sent(&format!("postgres://u:{}@h/db", auth::HIDDEN), "pg")).url, "postgres://u:real@h/db");
        assert_eq!(real_conn_in(&st, sent("postgres://u:typed@h/db", "pg")).url, "postgres://u:typed@h/db");
        assert_eq!(real_conn_in(&st, sent(&format!("postgres://u:{}@h/db", auth::HIDDEN), "other")).url, "postgres://u@h/db");
    }

    /// Five wrong passwords in a row rest the login; the right one doesn't get in meanwhile.
    #[test]
    fn wrong_passwords_rest_the_login() {
        let login = "lockout-test-person";
        for _ in 0..5 {
            assert!(auth::sign_in_allowed(login).is_ok());
            auth::sign_in_failed(login);
        }
        assert!(auth::sign_in_allowed(login).unwrap_err().contains("Too many"));
        auth::sign_in_ok(login);
        assert!(auth::sign_in_allowed(login).is_ok());
    }
}

#[cfg(test)]
fn uhnwi_fixed() -> Doc {
    let mut d = uhnwi_sample::template();
    d.params.start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1);
    d.params.date = chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
    d
}

