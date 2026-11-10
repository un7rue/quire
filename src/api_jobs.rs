//! The report API on the web server (backlog/report-api.md): other systems ask for reports with
//! an API account's key, and get PDFs back.
//!
//!     POST /api/v1/reports              a report for one client (`client_id`) or a batch (`clients`)
//!                                       → 202 and the job; checked first: a bad request is refused at once
//!     GET  /api/v1/jobs/{id}            the job: each report's status, pages, layout issues, values used
//!     GET  /api/v1/jobs/{id}/file       the PDF (one client), or a zip of the batch's PDFs
//!     GET  /api/v1/jobs/{id}/files/{client_id}   one client's PDF of a batch
//!     GET  /api/v1/templates            the reports this account may ask for, and what each accepts
//!
//! A request may name a `callback_url` (on a host Admin allows for the account): the finished job
//! is POSTed there, signed (src/api_delivery.rs), and tried again a few times if it isn't taken.
//! An account Admin gives a destination has each report copied there too.
//!
//! Requests carry `Authorization: Bearer <key>`; an `Idempotency-Key` header makes a retried request
//! return the job it made the first time. Jobs are kept in the data folder (`api/jobs/<id>/`), so a
//! restarted server finishes them; their files are deleted after Admin's retention days. Worker
//! threads make the reports, each with an engine of its own for drawing the pages. Every request and
//! every report made is in Activity, under the account (`api:<id>`).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::app::{draw, ApiDesk, PeriodAsked, Refusal, Request};
use crate::{api_accounts, api_delivery, audit, auth};

/// Clients in one request at most.
pub const MAX_CLIENTS: usize = 2000;
/// Requests per minute and account: new reports, and everything else (polling, downloads).
const POSTS_PER_MINUTE: u32 = 60;
const GETS_PER_MINUTE: u32 = 1200;
/// How often finished jobs past their retention are looked for.
const SWEEP_EVERY: Duration = Duration::from_secs(3600);
/// Waits before each try of a callback (seconds).
#[cfg(not(test))]
const CALLBACK_WAITS: [u64; 4] = [0, 10, 60, 300];
#[cfg(test)]
const CALLBACK_WAITS: [u64; 3] = [0, 0, 0];

/// An answer: status, type, body, extra headers.
pub struct Response {
    pub status: u16,
    pub kind: &'static str,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

impl Response {
    fn json(status: u16, v: &Json) -> Response {
        Response { status, kind: "application/json", body: v.to_string().into_bytes(), headers: vec![] }
    }
    fn error(status: u16, message: &str) -> Response {
        Response::json(status, &json!({"error": {"status": status, "message": message}}))
    }
    fn refused(r: &Refusal) -> Response {
        Response::error(r.status, &r.message)
    }
}

/// What a caller sends: a published report (UHNWI when left out), one client or several, the
/// period, the report's parameters.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobRequest {
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    clients: Vec<String>,
    period: PeriodAsked,
    #[serde(default)]
    params: BTreeMap<String, Json>,
    /// Only "pdf" for now.
    #[serde(default)]
    format: Option<String>,
    /// Called (POST, signed) when the job is done.
    #[serde(default)]
    callback_url: Option<String>,
}

/// A job's callback: where, and how it went.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Callback {
    url: String,
    /// pending, sent or failed
    status: String,
    attempts: u32,
    /// The last answer's status (0: nobody answered).
    code: u16,
    error: Option<String>,
    at: Option<String>,
}

/// A request accepted: its reports, one per client.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Job {
    id: String,
    /// The API account's id.
    account: String,
    created: String,
    finished: Option<String>,
    template: String,
    start: String,
    end: String,
    params: BTreeMap<String, Json>,
    /// Asked with `clients` (a zip), not `client_id` (a PDF).
    batch: bool,
    idempotency: Option<String>,
    items: Vec<Item>,
    #[serde(default)]
    callback: Option<Callback>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Item {
    client_id: String,
    client: String,
    /// queued, running, done or failed
    status: String,
    error: Option<Json>,
    pages: usize,
    issues: usize,
    /// The values it was made with.
    used: Json,
    /// The PDF in the job's folder, and the name it downloads as.
    file: Option<String>,
    name: String,
    bytes: u64,
    /// The copy to the account's destination: {status, at} or {status, error}.
    #[serde(default)]
    copy: Option<Json>,
}

impl Job {
    fn status(&self) -> &'static str {
        let n = |s: &str| self.items.iter().filter(|i| i.status == s).count();
        let (queued, running, done, failed) = (n("queued"), n("running"), n("done"), n("failed"));
        if queued + running > 0 {
            if running + done + failed > 0 { "running" } else { "queued" }
        } else if failed == 0 {
            "done"
        } else if done == 0 {
            "failed"
        } else {
            "partial"
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A job id nobody can guess.
fn new_id() -> String {
    let mut b = [0u8; 12];
    let _ = getrandom::getrandom(&mut b);
    format!("j{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

/// A name safe as a file name.
fn file_safe(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_alphanumeric() || " .-_—()".contains(c) { c } else { '-' }).collect();
    s.trim().chars().take(150).collect()
}

/// `filename*=` for a download's name (any letters), with a plain fallback.
fn disposition(name: &str, ext: &str) -> String {
    let enc: String = format!("{name}.{ext}").bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    format!("attachment; filename=\"report.{ext}\"; filename*=UTF-8''{enc}")
}

/// The report API: its jobs, their queue and workers.
pub struct ApiService {
    data: PathBuf,
    ui: PathBuf,
    jobs: Mutex<HashMap<String, Job>>,
    queue: Mutex<VecDeque<(String, usize)>>,
    ready: Condvar,
    /// (account, new reports?) → the minute's start and requests in it.
    rate: Mutex<HashMap<(String, bool), (Instant, u32)>>,
}

impl ApiService {
    /// Starts the service on the server's data folder, drawing with the UI code in `ui`: the jobs
    /// kept from before are read (unfinished ones queued again), `workers` threads make reports, and
    /// old files are deleted from time to time.
    pub fn start(data: &Path, ui: &Path, workers: usize) -> Arc<ApiService> {
        let svc = Arc::new(ApiService {
            data: data.to_path_buf(),
            ui: ui.to_path_buf(),
            jobs: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            rate: Mutex::new(HashMap::new()),
        });
        for id in svc.load() {
            svc.send_callback(&id);
        }
        for n in 0..workers.max(1) {
            let s = svc.clone();
            let _ = std::thread::Builder::new().name(format!("api-worker-{n}")).spawn(move || s.work());
        }
        let s = svc.clone();
        let _ = std::thread::Builder::new().name("api-sweeper".into()).spawn(move || loop {
            s.sweep(chrono::Local::now());
            std::thread::sleep(SWEEP_EVERY);
        });
        svc
    }

    fn dir(&self) -> PathBuf {
        self.data.join("api").join("jobs")
    }

    fn store(&self) -> auth::Store {
        auth::load(&self.data.join("users.json"))
    }

    /// The jobs in the data folder; reports not finished go back in the queue. Returns the finished
    /// jobs whose callback is still to be made.
    fn load(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.dir()) else { return vec![] };
        let mut jobs = lock(&self.jobs);
        let mut queue = lock(&self.queue);
        let mut found: Vec<Job> = rd.flatten().filter_map(|e| std::fs::read_to_string(e.path().join("job.json")).ok()).filter_map(|t| serde_json::from_str::<Job>(&t).ok()).collect();
        found.sort_by(|a, b| a.created.cmp(&b.created));
        let mut calls = Vec::new();
        for mut j in found {
            if j.finished.is_some() && j.callback.as_ref().is_some_and(|c| c.status == "pending") {
                calls.push(j.id.clone());
            }
            for (k, it) in j.items.iter_mut().enumerate() {
                if it.status == "queued" || it.status == "running" {
                    it.status = "queued".into();
                    queue.push_back((j.id.clone(), k));
                }
            }
            jobs.insert(j.id.clone(), j);
        }
        calls
    }

    /// Writes a job whole (never half a file).
    fn save(&self, j: &Job) {
        let d = self.dir().join(&j.id);
        if std::fs::create_dir_all(&d).is_err() {
            return;
        }
        let tmp = d.join("job.json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(j).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, d.join("job.json"));
        }
    }

    // ---------------------------------------------------------------- requests

    /// Answers one request to `/api/v1/…`.
    pub fn handle(&self, method: &str, path: &str, headers: &[(String, String)], body: &[u8]) -> Response {
        let header = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim());
        let Some(key) = header("authorization").and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer "))) else {
            let mut r = Response::error(401, "Send the API key: Authorization: Bearer <key>.");
            r.headers.push(("WWW-Authenticate".into(), "Bearer".into()));
            return r;
        };
        let st = self.store();
        let (account, session) = match api_accounts::sign_in(&st, key) {
            Ok(x) => x,
            Err(e) => {
                let mut r = Response::error(401, &e);
                r.headers.push(("WWW-Authenticate".into(), "Bearer error=\"invalid_token\"".into()));
                return r;
            }
        };
        let posting = method == "POST";
        if let Some(wait) = self.limited(&account.id, posting) {
            let mut r = Response::error(429, &format!("Too many requests: try again in {wait} s."));
            r.headers.push(("Retry-After".into(), wait.to_string()));
            return r;
        }
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').filter(|p| !p.is_empty()).collect();
        match (method, parts.as_slice()) {
            ("POST", ["api", "v1", "reports"]) => self.submit(&account, session, body, header("idempotency-key")),
            ("GET", ["api", "v1", "templates"]) => match ApiDesk::open(&self.data, session) {
                Ok(desk) => Response::json(200, &desk.templates()),
                Err(e) => Response::error(500, &e),
            },
            ("GET", ["api", "v1", "jobs", id]) => self.with_job(&account.id, id, |j| Response::json(200, &self.view(j))),
            ("GET", ["api", "v1", "jobs", id, "file"]) => self.with_job(&account.id, id, |j| self.file(j, None)),
            ("GET", ["api", "v1", "jobs", id, "files", client]) => self.with_job(&account.id, id, |j| self.file(j, Some(client))),
            (_, ["api", "v1", ..]) if !matches!(method, "GET" | "POST") => Response::error(405, "Use GET or POST."),
            _ => Response::error(404, "No such API path: see GET /api/v1/templates, POST /api/v1/reports, GET /api/v1/jobs/{id}."),
        }
    }

    /// Seconds to wait when the account is past its requests for the minute.
    fn limited(&self, account: &str, posting: bool) -> Option<u64> {
        let mut rate = lock(&self.rate);
        let e = rate.entry((account.to_string(), posting)).or_insert((Instant::now(), 0));
        if e.0.elapsed() >= Duration::from_secs(60) {
            *e = (Instant::now(), 0);
        }
        e.1 += 1;
        let max = if posting { POSTS_PER_MINUTE } else { GETS_PER_MINUTE };
        (e.1 > max).then(|| 60u64.saturating_sub(e.0.elapsed().as_secs()).max(1))
    }

    /// The account's own job (another account's is as good as missing).
    fn with_job(&self, account: &str, id: &str, f: impl FnOnce(&Job) -> Response) -> Response {
        let job = lock(&self.jobs).get(id).filter(|j| j.account == account).cloned();
        match job {
            Some(j) => f(&j),
            None => Response::error(404, &format!("No job “{id}” (or its files were deleted after the retention days).")),
        }
    }

    fn submit(&self, account: &api_accounts::ApiClient, session: auth::Session, body: &[u8], idem: Option<&str>) -> Response {
        let who = format!("api:{}", account.id);
        let log = |detail: &str, data: Json, err: Option<&str>| audit::record(Some(&self.data), &who, "api", "report request", &account.name, detail, data, err);
        let refuse = |r: Refusal, data: Json| {
            log(&format!("Refused: {}", r.message), data, Some(&r.message));
            Response::refused(&r)
        };
        let raw: Json = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return refuse(Refusal { status: 400, message: format!("The body isn't JSON: {e}") }, Json::Null),
        };
        let q: JobRequest = match serde_json::from_value(raw.clone()) {
            Ok(q) => q,
            Err(e) => return refuse(Refusal { status: 422, message: format!("The request doesn't fit: {e}") }, raw),
        };
        if q.format.as_deref().is_some_and(|f| f != "pdf") {
            return refuse(Refusal { status: 422, message: "The format is “pdf” (the only one for now).".into() }, raw);
        }
        let callback = match q.callback_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
            Some(url) => match api_accounts::callback_ok(account, url) {
                Ok(()) => Some(Callback { url: url.to_string(), status: "pending".into(), ..Default::default() }),
                Err(e) => return refuse(Refusal { status: 422, message: e }, raw),
            },
            None => None,
        };
        let idem = idem.filter(|k| !k.is_empty()).map(|k| k.chars().take(200).collect::<String>());
        if let Some(k) = &idem {
            let before = lock(&self.jobs).values().find(|j| j.account == account.id && j.idempotency.as_deref() == Some(k.as_str())).cloned();
            if let Some(j) = before {
                let mut r = Response::json(202, &self.view(&j));
                r.headers.push(("Idempotent-Replayed".into(), "true".into()));
                r.headers.push(("Location".into(), format!("/api/v1/jobs/{}", j.id)));
                return r;
            }
        }
        // the clients: one, or a batch (each once)
        let batch = q.client_id.is_none();
        let mut ids: Vec<String> = match (&q.client_id, q.clients.is_empty()) {
            (Some(c), true) => vec![c.trim().to_string()],
            (None, false) => q.clients.iter().map(|c| c.trim().to_string()).collect(),
            _ => return refuse(Refusal { status: 422, message: "Give either client_id (one report) or clients (a batch).".into() }, raw),
        };
        let mut seen = std::collections::HashSet::new();
        ids.retain(|c| seen.insert(c.clone()));
        if ids.len() > MAX_CLIENTS {
            return refuse(Refusal { status: 422, message: format!("{} clients: a request takes {MAX_CLIENTS} at most.", ids.len()) }, raw);
        }
        // everything that can be checked now is: a bad request is refused whole
        let desk = match ApiDesk::open(&self.data, session) {
            Ok(d) => d,
            Err(e) => return refuse(Refusal { status: 500, message: e }, raw),
        };
        let clients = match desk.clients() {
            Ok(c) => c,
            Err(r) => return refuse(r, raw),
        };
        let today = chrono::Local::now().date_naive();
        let mut items = Vec::new();
        let mut refused: Vec<(String, Refusal)> = Vec::new();
        let (mut template, mut start, mut end) = (String::new(), String::new(), String::new());
        for c in &ids {
            let req = Request { template: q.template.clone(), client_id: c.clone(), period: q.period.clone(), params: q.params.clone() };
            match desk.check(&req, today, Some(&clients)) {
                Ok(ok) => {
                    template = ok.template().to_string();
                    (start, end) = (ok.start.to_string(), ok.end.to_string());
                    items.push(Item { client_id: c.clone(), client: ok.client().to_string(), status: "queued".into(), ..Default::default() });
                }
                Err(r) => refused.push((c.clone(), r)),
            }
        }
        if let Some((_, first)) = refused.first() {
            let mut r = first.clone();
            if batch {
                r.message = format!("{} of {} clients can't be reported on; nothing was started. First: {}", refused.len(), ids.len(), first.message);
                log(&r.message, raw, Some(&r.message));
                let list: Vec<Json> = refused.iter().map(|(c, x)| json!({"client_id": c, "status": x.status, "message": x.message})).collect();
                return Response::json(r.status, &json!({"error": {"status": r.status, "message": r.message, "clients": list}}));
            }
            return refuse(r, raw);
        }
        let job = Job {
            id: new_id(),
            account: account.id.clone(),
            created: chrono::Local::now().to_rfc3339(),
            finished: None,
            template,
            start,
            end,
            params: q.params,
            batch,
            idempotency: idem,
            items,
            callback,
        };
        self.save(&job);
        let n = job.items.len();
        log(&format!("Asked for {n} report{}: {} · {} – {}", if n == 1 { "" } else { "s" }, job.template, job.start, job.end), json!({"job": job.id, "clients": ids, "params": job.params}), None);
        let view = self.view(&job);
        {
            let mut queue = lock(&self.queue);
            for k in 0..n {
                queue.push_back((job.id.clone(), k));
            }
            lock(&self.jobs).insert(job.id.clone(), job.clone());
        }
        self.ready.notify_all();
        let mut r = Response::json(202, &view);
        r.headers.push(("Location".into(), format!("/api/v1/jobs/{}", job.id)));
        r
    }

    /// A job as the caller sees it.
    fn view(&self, j: &Job) -> Json {
        let base = format!("/api/v1/jobs/{}", j.id);
        let keep = self.store().api.retention_days as i64;
        let expires = j.finished.as_deref().and_then(|f| chrono::DateTime::parse_from_rfc3339(f).ok()).map(|f| (f + chrono::Duration::days(keep)).to_rfc3339());
        let items: Vec<Json> = j.items.iter().map(|i| {
            let mut v = json!({"client_id": i.client_id, "client": i.client, "status": i.status});
            if i.status == "done" {
                v["pages"] = json!(i.pages);
                v["layout_issues"] = json!(i.issues);
                v["bytes"] = json!(i.bytes);
                v["values"] = i.used.clone();
                v["file"] = json!(format!("{base}/files/{}", i.client_id));
            }
            if let Some(e) = &i.error {
                v["error"] = e.clone();
            }
            if let Some(c) = &i.copy {
                v["copy"] = c.clone();
            }
            v
        }).collect();
        let status = j.status();
        let done = j.items.iter().filter(|i| i.status == "done").count();
        json!({
            "job_id": j.id, "status": status, "created": j.created, "finished": j.finished, "expires": expires,
            "template": j.template, "period": {"start": j.start, "end": j.end},
            "reports": j.items.len(), "done": done, "failed": j.items.iter().filter(|i| i.status == "failed").count(),
            "file": if done > 0 && (status == "done" || status == "partial") { json!(format!("{base}/file")) } else { Json::Null },
            "items": items,
            "callback": j.callback.as_ref().map(|c| json!({"url": c.url, "status": c.status, "attempts": c.attempts, "code": c.code, "error": c.error, "at": c.at})),
        })
    }

    /// The job's PDF (one client), a client's PDF of a batch, or the batch as a zip.
    fn file(&self, j: &Job, client: Option<&str>) -> Response {
        let dir = self.dir().join(&j.id);
        let pdf = |it: &Item| -> Response {
            match it.file.as_ref().and_then(|f| std::fs::read(dir.join(f)).ok()) {
                Some(bytes) => Response { status: 200, kind: "application/pdf", body: bytes, headers: vec![("Content-Disposition".into(), disposition(&it.name, "pdf"))] },
                None => Response::error(404, "The file was deleted after the retention days."),
            }
        };
        let not_yet = |it: &Item| match it.status.as_str() {
            "failed" => Response::json(it.error.as_ref().and_then(|e| e["status"].as_u64()).unwrap_or(500) as u16, &json!({"error": it.error})),
            s => Response::error(409, &format!("Not ready: the report is {s}. Ask again when the job is done.")),
        };
        if let Some(c) = client {
            return match j.items.iter().find(|i| i.client_id == c) {
                Some(it) if it.status == "done" => pdf(it),
                Some(it) => not_yet(it),
                None => Response::error(404, &format!("Client “{c}” isn't in this job.")),
            };
        }
        if !j.batch {
            return match j.items.first() {
                Some(it) if it.status == "done" => pdf(it),
                Some(it) => not_yet(it),
                None => Response::error(404, "The job has no report."),
            };
        }
        let status = j.status();
        if status == "queued" || status == "running" {
            return Response::error(409, &format!("Not ready: the job is {status}. Ask again when it is done."));
        }
        // the batch's PDFs in a zip
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for it in j.items.iter().filter(|i| i.status == "done") {
                let Some(bytes) = it.file.as_ref().and_then(|f| std::fs::read(dir.join(f)).ok()) else { continue };
                if zip.start_file(format!("{} — {}.pdf", file_safe(&it.client_id), file_safe(&it.name)), opts).is_err() || zip.write_all(&bytes).is_err() {
                    return Response::error(500, "Couldn't make the zip.");
                }
            }
            if zip.finish().is_err() {
                return Response::error(500, "Couldn't make the zip.");
            }
        }
        Response { status: 200, kind: "application/zip", body: buf.into_inner(), headers: vec![("Content-Disposition".into(), disposition(&format!("Quire {} {}", j.template, j.end), "zip"))] }
    }

    // ---------------------------------------------------------------- workers

    fn work(self: &Arc<Self>) {
        loop {
            let task = {
                let mut q = lock(&self.queue);
                loop {
                    if let Some(t) = q.pop_front() {
                        break t;
                    }
                    q = self.ready.wait(q).unwrap_or_else(|e| e.into_inner());
                }
            };
            self.run(&task.0, task.1);
        }
    }

    /// Makes one report of a job.
    fn run(self: &Arc<Self>, id: &str, k: usize) {
        let job = {
            let mut jobs = lock(&self.jobs);
            let Some(j) = jobs.get_mut(id) else { return };
            let Some(it) = j.items.get_mut(k) else { return };
            if it.status != "queued" {
                return;
            }
            it.status = "running".into();
            j.clone()
        };
        self.save(&job);
        let item = job.items[k].clone();
        let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.make(&job, &item))).unwrap_or_else(|_| Err(Refusal { status: 500, message: "The report failed to draw.".into() }));
        let who = format!("api:{}", job.account);
        let mut file = None;
        let mut target = None;
        let result = made.and_then(|(drawn, used, to)| {
            let f = format!("{:04}-{}.pdf", k + 1, file_safe(&item.client_id));
            std::fs::write(self.dir().join(&job.id).join(&f), &drawn.pdf).map_err(|e| Refusal { status: 500, message: format!("Couldn't keep the file: {e}") })?;
            file = Some(f);
            target = to;
            Ok((drawn, used))
        });
        // the copy to the account's destination (a failed copy leaves the report downloadable)
        let copy = match (&result, &target) {
            (Ok((drawn, _)), Some(t)) => {
                let day = job.created.get(..10).unwrap_or("");
                let rel = if t.kind == "s3" {
                    format!("{}/{day}/{}/{}.pdf", job.account, job.id, api_delivery::safe_name(&item.client_id))
                } else {
                    format!("{}/{day}/{}/{} — {}.pdf", job.account, job.id, api_delivery::safe_name(&item.client_id), api_delivery::safe_name(&drawn.name))
                };
                let r = api_delivery::copy(t, &rel, &drawn.pdf);
                if let Err(e) = &r {
                    audit::record(Some(&self.data), &who, "api", "api copy", &item.client, &format!("Copy to “{}” failed", t.name), json!({"job": job.id, "client_id": item.client_id, "to": t.place()}), Some(e));
                }
                Some(match r {
                    Ok(at) => json!({"status": "done", "to": t.name, "at": at}),
                    Err(e) => json!({"status": "failed", "to": t.name, "error": e}),
                })
            }
            _ => None,
        };
        let mut jobs = lock(&self.jobs);
        let Some(j) = jobs.get_mut(id) else { return };
        let it = &mut j.items[k];
        match &result {
            Ok((drawn, used)) => {
                it.status = "done".into();
                it.pages = drawn.pages;
                it.issues = drawn.issues;
                it.bytes = drawn.pdf.len() as u64;
                it.used = used.clone();
                it.name = drawn.name.clone();
                it.file = file;
                it.copy = copy;
                audit::record(Some(&self.data), &who, "document", "api export", &item.client,
                    &format!("API report · {} · {} pages{}", item.client, drawn.pages, if drawn.issues > 0 { format!(" · {} layout issue(s)", drawn.issues) } else { String::new() }),
                    json!({"job": job.id, "client": item.client, "client_id": item.client_id, "template": job.template, "period": {"start": job.start, "end": job.end}, "format": "pdf"}), None);
            }
            Err(r) => {
                it.status = "failed".into();
                it.error = Some(json!({"status": r.status, "message": r.message}));
                audit::record(Some(&self.data), &who, "document", "api export", &item.client, &format!("API report · {} · failed", item.client),
                    json!({"job": job.id, "client_id": item.client_id, "template": job.template}), Some(&r.message));
            }
        }
        let finished = !j.items.iter().any(|i| i.status == "queued" || i.status == "running");
        if finished {
            j.finished = Some(chrono::Local::now().to_rfc3339());
        }
        let snapshot = j.clone();
        drop(jobs);
        self.save(&snapshot);
        if finished && snapshot.callback.is_some() {
            self.send_callback(id);
        }
    }

    /// Calls the job's callback back (on a thread of its own), trying again a few times; the
    /// outcome is kept with the job and in Activity.
    fn send_callback(self: &Arc<Self>, id: &str) {
        let (svc, id) = (self.clone(), id.to_string());
        let _ = std::thread::Builder::new().name("api-callback".into()).spawn(move || {
            for wait in CALLBACK_WAITS {
                std::thread::sleep(Duration::from_secs(wait));
                let Some(job) = lock(&svc.jobs).get(&id).cloned() else { return };
                let Some(cb) = job.callback.clone().filter(|c| c.status == "pending") else { return };
                // signed with the account's key as it is now
                let account = svc.store().api.clients.into_iter().find(|c| c.id == job.account && c.active && !c.sign_key.is_empty());
                let r = match &account {
                    Some(a) => api_delivery::call_back(&cb.url, &a.sign_key, &job.id, &svc.view(&job).to_string()),
                    None => Err("The API account was switched off or removed.".into()),
                };
                let last = cb.attempts + 1 >= CALLBACK_WAITS.len() as u32 || account.is_none();
                let mut jobs = lock(&svc.jobs);
                let Some(j) = jobs.get_mut(&id) else { return };
                let Some(c) = j.callback.as_mut() else { return };
                c.attempts += 1;
                c.at = Some(chrono::Local::now().to_rfc3339());
                match r {
                    Ok(code) if (200..300).contains(&code) => {
                        c.status = "sent".into();
                        c.code = code;
                        c.error = None;
                    }
                    Ok(code) => {
                        c.code = code;
                        c.error = Some(format!("It answered {code}."));
                    }
                    Err(e) => {
                        c.code = 0;
                        c.error = Some(e);
                    }
                }
                if c.status == "pending" && last {
                    c.status = "failed".into();
                }
                let done = c.status != "pending";
                let c = c.clone();
                let snapshot = j.clone();
                drop(jobs);
                svc.save(&snapshot);
                if done {
                    audit::record(Some(&svc.data), &format!("api:{}", snapshot.account), "api", "api callback", &c.url,
                        &format!("Callback {} after {} tr{}", if c.status == "sent" { "delivered" } else { "failed" }, c.attempts, if c.attempts == 1 { "y" } else { "ies" }),
                        json!({"job": snapshot.id, "url": c.url, "code": c.code}), c.error.as_deref().filter(|_| c.status != "sent"));
                    return;
                }
            }
        });
    }

    /// The report itself, as the job's account (as it is now: switched off or removed, it stops),
    /// with the destination it copies to.
    fn make(&self, job: &Job, item: &Item) -> Result<(crate::app::Drawn, Json, Option<api_accounts::ApiTarget>), Refusal> {
        let st = self.store();
        let account = st.api.clients.iter().find(|c| c.id == job.account && c.active)
            .ok_or_else(|| Refusal { status: 403, message: "The API account was switched off or removed.".into() })?;
        let desk = ApiDesk::open(&self.data, account.session()).map_err(|e| Refusal { status: 500, message: e })?;
        let req = Request {
            template: Some(job.template.clone()),
            client_id: item.client_id.clone(),
            period: PeriodAsked::Dates { start: job.start.clone(), end: job.end.clone() },
            params: job.params.clone(),
        };
        let made = desk.make(&req, chrono::Local::now().date_naive())?;
        let drawn = draw(&self.ui, &made)?;
        let to = st.api.targets.iter().find(|t| !account.copy_to.is_empty() && t.id == account.copy_to).cloned();
        Ok((drawn, made.used, to))
    }

    // ---------------------------------------------------------------- retention

    /// Deletes jobs finished longer ago than Admin's retention days, files and all.
    fn sweep(&self, now: chrono::DateTime<chrono::Local>) -> usize {
        let keep = chrono::Duration::days(self.store().api.retention_days.max(1) as i64);
        let old: Vec<String> = lock(&self.jobs).values()
            .filter(|j| j.finished.as_deref().and_then(|f| chrono::DateTime::parse_from_rfc3339(f).ok()).is_some_and(|f| now.fixed_offset() - f > keep))
            .map(|j| j.id.clone()).collect();
        for id in &old {
            lock(&self.jobs).remove(id);
            let _ = std::fs::remove_dir_all(self.dir().join(id));
        }
        old.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A data folder with an API account (one banker's clients) and its key; the UHNWI report takes
    /// products over the API.
    fn server() -> (PathBuf, String) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("quire_api_jobs_{}_{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut st = auth::seed();
        let e = api_accounts::ApiClientEdit { name: "CRM".into(), active: true, access: auth::Access { all: false, coverage: vec!["S0001".into()], ..Default::default() }, ..Default::default() };
        let key = api_accounts::save(&mut st, e, "admin").unwrap().unwrap();
        let mut r = api_accounts::ApiReport::default();
        r.params.insert("product".into(), api_accounts::ApiParam { accept: true, default: Json::Null });
        st.api.reports.insert("uhnwi".into(), r);
        auth::save(&dir.join("users.json"), &st).unwrap();
        (dir, key)
    }

    fn ui() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
    }

    fn call(svc: &ApiService, key: &str, method: &str, path: &str, body: Json) -> (u16, Json, Response) {
        let h = vec![("Authorization".to_string(), format!("Bearer {key}"))];
        let r = svc.handle(method, path, &h, if body.is_null() { b"" } else { Box::leak(body.to_string().into_bytes().into_boxed_slice()) });
        let j = if r.kind == "application/json" { serde_json::from_slice(&r.body).unwrap_or(Json::Null) } else { Json::Null };
        (r.status, j, r)
    }

    /// Polls a job until it is finished (a minute at most).
    fn wait(svc: &ApiService, key: &str, id: &str) -> Json {
        for _ in 0..600 {
            let (_, j, _) = call(svc, key, "GET", &format!("/api/v1/jobs/{id}"), Json::Null);
            if !matches!(j["status"].as_str(), Some("queued" | "running")) {
                return j;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("the job didn't finish");
    }

    /// The bank's ids of the account's clients (banker S0001's).
    fn own_clients(dir: &Path) -> Vec<String> {
        let st = auth::load(&dir.join("users.json"));
        let a = auth::allowed(&st, &st.api.clients[0].access).unwrap().unwrap();
        a.into_iter().map(|(id, _)| id).collect()
    }

    /// One report: 202 and the job; it finishes; the PDF downloads; the values used come back; a
    /// retried request with the same Idempotency-Key gets the same job; Activity has the request and
    /// the report under the account.
    #[test]
    fn a_report_is_made_and_downloaded() {
        if crate::dbconn::demo_path().is_none() { return }
        let (dir, key) = server();
        let svc = ApiService::start(&dir, &ui(), 2);
        let c = own_clients(&dir)[0].clone();
        let body = json!({"client_id": c, "period": {"start": "2026-07-01", "end": "2026-09-30"}, "params": {"product": ["BROK", "BANK"]}});
        let h = vec![("Authorization".to_string(), format!("Bearer {key}")), ("Idempotency-Key".to_string(), "abc-1".to_string())];
        let r = svc.handle("POST", "/api/v1/reports", &h, body.to_string().as_bytes());
        assert_eq!(r.status, 202, "{}", String::from_utf8_lossy(&r.body));
        let j: Json = serde_json::from_slice(&r.body).unwrap();
        let id = j["job_id"].as_str().unwrap().to_string();
        assert!(r.headers.iter().any(|(k, v)| k == "Location" && v.ends_with(&id)));
        assert_eq!(j["reports"], 1);
        // the same request again: the same job
        let again = svc.handle("POST", "/api/v1/reports", &h, body.to_string().as_bytes());
        assert_eq!(serde_json::from_slice::<Json>(&again.body).unwrap()["job_id"], id.as_str());
        assert!(again.headers.iter().any(|(k, _)| k == "Idempotent-Replayed"));
        let j = wait(&svc, &key, &id);
        assert_eq!(j["status"], "done", "{j}");
        let it = &j["items"][0];
        assert!(it["pages"].as_u64().unwrap() > 10);
        assert_eq!(it["values"]["params"]["product"], json!(["BROK", "BANK"]));
        assert_eq!(it["values"]["period"]["end"], "2026-09-30");
        assert!(j["expires"].as_str().is_some());
        let (s, _, f) = call(&svc, &key, "GET", &format!("/api/v1/jobs/{id}/file"), Json::Null);
        assert_eq!(s, 200);
        assert!(f.kind == "application/pdf" && f.body.starts_with(b"%PDF") && f.body.len() > 50_000);
        assert!(f.headers.iter().any(|(k, v)| k == "Content-Disposition" && v.contains("filename*=UTF-8''")));
        let (s, _, f2) = call(&svc, &key, "GET", &format!("/api/v1/jobs/{id}/files/{c}"), Json::Null);
        assert!(s == 200 && f2.body == f.body);
        // Activity: the request and the report, under the account
        let log = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains("activity") || e.path().extension().is_some_and(|x| x == "jsonl"))
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default()).collect::<String>();
        let log = if log.is_empty() { walk(&dir) } else { log };
        assert!(log.contains("api:crm") && log.contains("report request") && log.contains("api export"), "{}", &log[..log.len().min(400)]);
        assert!(!log.contains(key.rsplit('.').next().unwrap()), "no key in the log");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every file's text under `dir` (the activity log's place may change).
    fn walk(dir: &Path) -> String {
        let mut out = String::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                out += &walk(&p);
            } else if p.extension().is_some_and(|x| x == "jsonl" || x == "log") {
                out += &std::fs::read_to_string(&p).unwrap_or_default();
            }
        }
        out
    }

    /// A batch: one job, a report per client, a zip of the PDFs; a client of another banker refuses
    /// the whole batch, naming it.
    #[test]
    fn a_batch_makes_a_zip() {
        if crate::dbconn::demo_path().is_none() { return }
        let (dir, key) = server();
        let svc = ApiService::start(&dir, &ui(), 2);
        let mine = own_clients(&dir);
        let two = vec![mine[0].clone(), mine[1].clone(), mine[0].clone()];
        let (s, j, _) = call(&svc, &key, "POST", "/api/v1/reports", json!({"clients": two, "period": {"preset": "last_quarter"}}));
        assert_eq!(s, 202, "{j}");
        assert_eq!(j["reports"], 2, "each client once");
        let id = j["job_id"].as_str().unwrap().to_string();
        // not ready yet, or ready: never a half zip
        let (s, _, _) = call(&svc, &key, "GET", &format!("/api/v1/jobs/{id}/file"), Json::Null);
        assert!(s == 409 || s == 200);
        let j = wait(&svc, &key, &id);
        assert_eq!(j["status"], "done", "{j}");
        let (s, _, z) = call(&svc, &key, "GET", &format!("/api/v1/jobs/{id}/file"), Json::Null);
        assert_eq!(s, 200);
        assert_eq!(z.kind, "application/zip");
        let zip = zip::ZipArchive::new(std::io::Cursor::new(z.body)).unwrap();
        assert_eq!(zip.len(), 2);
        assert!(zip.file_names().all(|n| n.ends_with(".pdf")));
        // another banker's client: nothing starts, and the refusal names it
        let st = auth::load(&dir.join("users.json"));
        let other = auth::options(&st).unwrap()["clients"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_string()).find(|c| !mine.contains(c)).unwrap();
        let (s, j, _) = call(&svc, &key, "POST", "/api/v1/reports", json!({"clients": [mine[0], other], "period": {"preset": "ytd"}}));
        assert_eq!(s, 403, "{j}");
        assert_eq!(j["error"]["clients"][0]["client_id"], other.as_str());
        assert_eq!(lock(&svc.jobs).len(), 1, "no job for a refused batch");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What is refused, and with which status: no key or a wrong one (401), a malformed body (400),
    /// a request that doesn't fit (422), an unknown client (404), another account's job (404).
    #[test]
    fn bad_requests_are_refused() {
        let (dir, key) = server();
        let svc = ApiService::start(&dir, &ui(), 1);
        assert_eq!(svc.handle("GET", "/api/v1/templates", &[], b"").status, 401);
        assert_eq!(call(&svc, "qk.crm.wrong", "GET", "/api/v1/templates", Json::Null).0, 401);
        let r = svc.handle("POST", "/api/v1/reports", &[("Authorization".into(), format!("Bearer {key}"))], b"{not json");
        assert_eq!(r.status, 400);
        let per = json!({"preset": "last_quarter"});
        assert_eq!(call(&svc, &key, "POST", "/api/v1/reports", json!({"period": per})).0, 422, "no client");
        assert_eq!(call(&svc, &key, "POST", "/api/v1/reports", json!({"client_id": "C0001", "clients": ["C0002"], "period": per})).0, 422, "both");
        assert_eq!(call(&svc, &key, "POST", "/api/v1/reports", json!({"client_id": "C0001", "period": per, "format": "pptx"})).0, 422);
        assert_eq!(call(&svc, &key, "POST", "/api/v1/reports", json!({"client_id": "C0001", "period": per, "colour": 1})).0, 422);
        if crate::dbconn::demo_path().is_some() {
            assert_eq!(call(&svc, &key, "POST", "/api/v1/reports", json!({"client_id": "C9999", "period": per})).0, 404);
            // what it may ask for
            let (s, t, _) = call(&svc, &key, "GET", "/api/v1/templates", Json::Null);
            assert_eq!(s, 200);
            assert_eq!(t["templates"][0]["id"], "uhnwi");
            assert_eq!(t["templates"][0]["params"][0]["name"], "product");
        }
        assert_eq!(call(&svc, &key, "GET", "/api/v1/jobs/jnope", Json::Null).0, 404);
        assert_eq!(call(&svc, &key, "GET", "/api/v1/nothing", Json::Null).0, 404);
        // another account's job is as good as missing
        lock(&svc.jobs).insert("jother".into(), Job { id: "jother".into(), account: "someone".into(), created: String::new(), finished: None, template: "uhnwi".into(),
            start: String::new(), end: String::new(), params: BTreeMap::new(), batch: false, idempotency: None, items: vec![], callback: None });
        assert_eq!(call(&svc, &key, "GET", "/api/v1/jobs/jother", Json::Null).0, 404);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Too many new reports in a minute: 429 with Retry-After.
    #[test]
    fn requests_are_limited() {
        let (dir, key) = server();
        let svc = ApiService::start(&dir, &ui(), 1);
        let mut last = 0;
        for _ in 0..=POSTS_PER_MINUTE {
            last = call(&svc, &key, "POST", "/api/v1/reports", json!({"period": {"preset": "ytd"}})).0;
        }
        assert_eq!(last, 429);
        let r = call(&svc, &key, "POST", "/api/v1/reports", json!({})).2;
        assert!(r.headers.iter().any(|(k, _)| k == "Retry-After"));
        assert_eq!(call(&svc, &key, "GET", "/api/v1/jobs/jx", Json::Null).0, 404, "reading isn't limited with it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Jobs are kept: a restarted service has them and finishes what was left; finished jobs past the
    /// retention days are deleted, files and all.
    #[test]
    fn jobs_outlive_a_restart_and_expire() {
        let (dir, key) = server();
        let svc = ApiService::start(&dir, &ui(), 1);
        // a job written as a stopped server left it: one report still queued, for an account that is gone
        let job = Job { id: "jkept".into(), account: "crm".into(), created: chrono::Local::now().to_rfc3339(), finished: None, template: "uhnwi".into(),
            start: "2026-01-01".into(), end: "2026-03-31".into(), params: BTreeMap::new(), batch: false, idempotency: None,
            items: vec![Item { client_id: "C0001".into(), client: "X".into(), status: "running".into(), ..Default::default() }], callback: None };
        svc.save(&job);
        let mut st = auth::load(&dir.join("users.json"));
        st.api.clients[0].active = false;
        auth::save(&dir.join("users.json"), &st).unwrap();
        drop(svc);
        let svc = ApiService::start(&dir, &ui(), 1);
        // it runs again (and stops: the account is switched off)
        for _ in 0..100 {
            if lock(&svc.jobs)["jkept"].finished.is_some() { break }
            std::thread::sleep(Duration::from_millis(50));
        }
        let j = lock(&svc.jobs)["jkept"].clone();
        assert_eq!(j.items[0].status, "failed");
        assert_eq!(j.items[0].error.as_ref().unwrap()["status"], 403);
        // past the retention days
        assert_eq!(svc.sweep(chrono::Local::now()), 0, "kept within the days");
        assert_eq!(svc.sweep(chrono::Local::now() + chrono::Duration::days(8)), 1);
        assert!(!dir.join("api/jobs/jkept").exists());
        let _ = key;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A job with a callback and a destination: each report is copied there, and when the job is
    /// done the callback gets it, signed with the account's key; a callback to a host Admin didn't
    /// allow is refused at once.
    #[test]
    fn reports_are_copied_and_called_back() {
        if crate::dbconn::demo_path().is_none() { return }
        let (dir, key) = server();
        let (hook, seen) = crate::api_delivery::tests::stand_in(200);
        let share = dir.join("share");
        let mut st = auth::load(&dir.join("users.json"));
        let t = api_accounts::save_target(&mut st, api_accounts::ApiTargetEdit { name: "Share".into(), kind: "folder".into(), folder: share.display().to_string(), ..Default::default() }, "admin").unwrap();
        st.api.clients[0].copy_to = t;
        st.api.clients[0].callback_hosts = vec!["127.0.0.1".into()];
        auth::save(&dir.join("users.json"), &st).unwrap();
        let svc = ApiService::start(&dir, &ui(), 2);
        let mine = own_clients(&dir);
        let (s, j, _) = call(&svc, &key, "POST", "/api/v1/reports", json!({"clients": [mine[0], mine[1]], "period": {"preset": "last_quarter"}, "callback_url": format!("{hook}/quire")}));
        assert_eq!(s, 202, "{j}");
        let id = j["job_id"].as_str().unwrap().to_string();
        let j = wait(&svc, &key, &id);
        assert_eq!(j["status"], "done", "{j}");
        // copied: one PDF per client, under the account, the day and the job
        for it in j["items"].as_array().unwrap() {
            assert_eq!(it["copy"]["status"], "done", "{it}");
            let at = it["copy"]["at"].as_str().unwrap();
            assert!(at.contains(&format!("crm/{}/{id}/", &chrono::Local::now().format("%Y-%m-%d"))) || at.contains("/crm/"), "{at}");
            assert!(std::fs::read(at).unwrap().starts_with(b"%PDF"));
        }
        // called back once, signed
        for _ in 0..100 {
            if !seen.lock().unwrap().is_empty() { break }
            std::thread::sleep(Duration::from_millis(50));
        }
        let got = seen.lock().unwrap().first().cloned().expect("the callback came");
        let h = |n: &str| got.2.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or_default();
        assert_eq!((got.0.as_str(), got.1.as_str()), ("POST", "/quire"));
        let body = String::from_utf8(got.3.clone()).unwrap();
        let ts: i64 = h("x-quire-timestamp").parse().unwrap();
        assert_eq!(h("x-quire-signature"), crate::api_delivery::signature(&api_accounts::sign_key_of(&key), ts, &body), "the caller can check it with its key");
        let sent: Json = serde_json::from_str(&body).unwrap();
        assert_eq!((sent["job_id"].as_str(), sent["status"].as_str()), (Some(id.as_str()), Some("done")));
        for _ in 0..100 {
            if lock(&svc.jobs)[&id].callback.as_ref().unwrap().status == "sent" { break }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(call(&svc, &key, "GET", &format!("/api/v1/jobs/{id}"), Json::Null).1["callback"]["status"], "sent");
        // a host Admin didn't allow
        let (s, j, _) = call(&svc, &key, "POST", "/api/v1/reports", json!({"client_id": mine[0], "period": {"preset": "ytd"}, "callback_url": "https://elsewhere.example/hook"}));
        assert_eq!(s, 422, "{j}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A callback nobody takes is tried again, then marked failed.
    #[test]
    fn a_callback_not_taken_is_tried_again() {
        let (dir, key) = server();
        let (hook, seen) = crate::api_delivery::tests::stand_in(500);
        let mut st = auth::load(&dir.join("users.json"));
        st.api.clients[0].callback_hosts = vec!["127.0.0.1".into()];
        auth::save(&dir.join("users.json"), &st).unwrap();
        let svc = ApiService::start(&dir, &ui(), 1);
        let job = Job { id: "jcb".into(), account: "crm".into(), created: chrono::Local::now().to_rfc3339(), finished: Some(chrono::Local::now().to_rfc3339()), template: "uhnwi".into(),
            start: "2026-01-01".into(), end: "2026-03-31".into(), params: BTreeMap::new(), batch: false, idempotency: None, items: vec![],
            callback: Some(Callback { url: format!("{hook}/x"), status: "pending".into(), ..Default::default() }) };
        lock(&svc.jobs).insert(job.id.clone(), job);
        svc.send_callback("jcb");
        for _ in 0..200 {
            if lock(&svc.jobs)["jcb"].callback.as_ref().unwrap().status != "pending" { break }
            std::thread::sleep(Duration::from_millis(25));
        }
        let c = lock(&svc.jobs)["jcb"].callback.clone().unwrap();
        assert_eq!((c.status.as_str(), c.attempts, c.code), ("failed", CALLBACK_WAITS.len() as u32, 500));
        assert_eq!(seen.lock().unwrap().len(), CALLBACK_WAITS.len());
        let _ = key;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
