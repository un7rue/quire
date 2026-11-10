//! Quire in the browser: serves the same `ui/` as the desktop app, and runs its commands
//! (`POST /api/call` with `{cmd, args}`) through the same core.
//!
//!     cargo run --no-default-features --features render --bin quire-server -- [--port 8766] [--data DIR] [--ui DIR]
//!         [--host 127.0.0.1] [--allow-dir DIR]… [--allow-host HOST]… [--api-workers N] [--secure] [--dev]
//!
//! Each browser gets a workspace of its own (a cookie): its own sign-in, open documents, undo
//! and data. Each person's settings and files are kept apart in the data folder:
//!
//!     users.json, templates/, reports/, themes/, uploads/   shared, as in the desktop app's folder
//!     users/<login>/settings.json                           each person's settings (their Python too)
//!     files/<login>/                                        each person's saved reports and exports
//!     secrets.json                                          AI keys and database passwords, sealed
//!                                                           with QUIRE_SECRET_KEY (else master.key)
//!
//! Data sources read files only from the data folder and `--allow-dir` folders, and connect only
//! to `--allow-host` servers when any are given. `--secure`: the cookie only travels over HTTPS
//! (run it behind an HTTPS proxy).
//!
//! The report API (`/api/v1/…`, src/api_jobs.rs): other systems ask for reports with an API
//! account's key (Admin → API). Built with `--features render` (the pages are drawn in V8);
//! `--api-workers N` reports are made at once (2).
//!
//! `--dev` lets pages from other local ports call it (the headless checks in dev/shots); they
//! share one workspace, as they can't carry the cookie.

use quire::{AppState, FileSpec, Host};
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The report API, when the server is built with the renderer.
#[cfg(feature = "render")]
type Api = quire::ApiService;
#[cfg(not(feature = "render"))]
struct Api;

/// A workspace left alone this long is closed (its documents with it).
const IDLE: Duration = Duration::from_secs(12 * 3600);
/// Workspaces kept at most; past it the longest idle goes first.
const MAX_SESSIONS: usize = 1000;
const COOKIE: &str = "quire_sid";
/// The largest request (an upload of a 200 MB file, as base64).
const MAX_BODY: usize = 300_000_000;
/// Connections served at once; past it the server answers "busy".
const MAX_CONNECTIONS: usize = 512;
/// A request that stalls this long is dropped.
const TIMEOUT: Duration = Duration::from_secs(120);

/// One workspace's platform: the server's data folder, and whose settings and files are in front.
struct Web {
    data: PathBuf,
    login: Mutex<Option<String>>,
}

/// A login as a folder name: letters, digits, `.`, `-`, `_`.
fn folder(login: &str) -> String {
    let s: String = login.chars().map(|c| if c.is_alphanumeric() || ".-_".contains(c) { c } else { '_' }).collect();
    if s.is_empty() || s.starts_with('.') { format!("_{s}") } else { s }
}

impl Web {
    fn who(&self) -> Option<String> {
        self.login.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The person's own folder for what the browser version saves (reports, exports, batch files).
    fn files(&self) -> Result<PathBuf, String> {
        let who = self.who().ok_or("Sign in first.")?;
        let d = self.data.join("files").join(folder(&who));
        std::fs::create_dir_all(&d).map_err(|e| format!("Could not make {}: {e}", d.display()))?;
        Ok(d)
    }
}

impl Host for Web {
    fn data_dir(&self) -> Option<PathBuf> {
        Some(self.data.clone())
    }

    fn pick_file(&self, _d: &FileSpec) -> Result<Option<PathBuf>, String> {
        // the page picks and uploads files itself (`upload_file`); this is what has no upload yet
        Err("This file can't be picked in the browser version yet. Use the desktop app for it.".into())
    }

    /// Saved into the person's folder under the suggested name. A document never replaces another
    /// one of the same name (it gets " (2)"); an export does (it is downloaded anyway).
    fn save_file(&self, d: &FileSpec) -> Result<Option<PathBuf>, String> {
        let ext = d.filters.first().and_then(|(_, e)| e.first()).cloned().unwrap_or_default();
        let name = d.name.clone().unwrap_or_else(|| format!("Quire.{ext}"));
        // only the file's name: never a folder of the browser's choosing
        let name = Path::new(&name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Quire".into());
        let dir = self.files()?;
        let doc = matches!(ext.as_str(), "rpt" | "rtpl" | "qtheme");
        let (stem, dot) = match name.rsplit_once('.') { Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")), _ => (name.clone(), String::new()) };
        let path = (1..).map(|n| dir.join(if n == 1 { name.clone() } else { format!("{stem} ({n}){dot}") })).find(|p| !doc || !p.exists()).unwrap();
        Ok(Some(path))
    }

    fn pick_folder(&self) -> Result<Option<PathBuf>, String> {
        self.files().map(Some)
    }

    fn pick_color(&self) -> Result<Option<String>, String> {
        Err("The screen eyedropper is in the desktop app.".into())
    }

    fn reveal(&self, _path: &Path, _open: bool) -> Result<(), String> {
        Err("Showing files in Finder is in the desktop app.".into())
    }

    fn signed_in(&self, login: Option<&str>) {
        *self.login.lock().unwrap_or_else(|e| e.into_inner()) = login.map(String::from);
    }

    /// Each person's own; the first time, a copy of the server's (its defaults).
    fn settings_file(&self) -> Option<PathBuf> {
        let shared = self.data.join("settings.json");
        let Some(who) = self.who() else { return Some(shared) };
        let mine = self.data.join("users").join(folder(&who)).join("settings.json");
        if !mine.exists() && shared.exists() {
            let _ = std::fs::create_dir_all(mine.parent()?);
            let _ = std::fs::copy(&shared, &mine);
        }
        Some(mine)
    }

    fn files_dir(&self) -> Option<PathBuf> {
        self.files().ok()
    }

    /// Only documents inside the server's data folder (the libraries, the person's files, uploads).
    fn may_open(&self, path: &Path) -> bool {
        match (path.canonicalize(), self.data.canonicalize()) {
            (Ok(p), Ok(d)) => p.starts_with(d),
            _ => false,
        }
    }
}

/// The open workspaces, by cookie.
struct Sessions {
    data: PathBuf,
    map: Mutex<HashMap<String, (Arc<AppState>, Instant)>>,
    swept: Mutex<Instant>,
}

impl Sessions {
    /// The workspace of a request's cookie, or a new one (with the cookie to set).
    fn get(&self, id: Option<&str>) -> (Arc<AppState>, Option<String>) {
        self.sweep();
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((core, last)) = id.and_then(|id| map.get_mut(id)) {
            *last = Instant::now();
            return (core.clone(), None);
        }
        // a cookie the server no longer knows (restarted, expired) is replaced, never reused
        let id = match id { Some("dev") => "dev".to_string(), _ => new_id() };
        let core = Arc::new(AppState::new(Box::new(Web { data: self.data.clone(), login: Mutex::new(None) }), None));
        map.insert(id.clone(), (core.clone(), Instant::now()));
        (core, Some(id))
    }

    /// Closes workspaces left idle (every few minutes), and the longest idle past the limit.
    fn sweep(&self) {
        {
            let mut s = self.swept.lock().unwrap_or_else(|e| e.into_inner());
            if s.elapsed() < Duration::from_secs(300) && self.map.lock().map(|m| m.len() < MAX_SESSIONS).unwrap_or(true) {
                return;
            }
            *s = Instant::now();
        }
        let gone: Vec<Arc<AppState>> = {
            let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
            let mut old: Vec<String> = map.iter().filter(|(_, (_, t))| t.elapsed() > IDLE).map(|(k, _)| k.clone()).collect();
            if map.len() - old.len() >= MAX_SESSIONS {
                let mut by_age: Vec<(String, Instant)> = map.iter().filter(|(k, _)| !old.contains(k)).map(|(k, (_, t))| (k.clone(), *t)).collect();
                by_age.sort_by_key(|(_, t)| *t);
                old.extend(by_age.into_iter().take(map.len() - old.len() - MAX_SESSIONS + 1).map(|(k, _)| k));
            }
            old.iter().filter_map(|k| map.remove(k)).map(|(c, _)| c).collect()
        };
        for core in gone {
            core.end_work("session closed");
        }
    }
}

/// A session id nobody can guess: 128 random bits.
fn new_id() -> String {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).expect("no randomness");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

struct Opts {
    host: String,
    port: u16,
    data: PathBuf,
    ui: PathBuf,
    dev: bool,
    secure: bool,
    allow_dirs: Vec<PathBuf>,
    allow_hosts: Vec<String>,
    api_workers: usize,
}

fn opts() -> Opts {
    let mut o = Opts {
        host: "127.0.0.1".into(),
        port: 8766,
        // QUIRE_BRIDGE_DIR: what the dev bridge used, so the screenshot steps keep working
        data: std::env::var("QUIRE_DATA").or_else(|_| std::env::var("QUIRE_BRIDGE_DIR")).map(PathBuf::from).unwrap_or_else(|_| {
            std::env::var("HOME").map(|h| PathBuf::from(h).join(".quire-server")).unwrap_or_else(|_| std::env::temp_dir().join("quire-server"))
        }),
        ui: std::env::var("QUIRE_UI").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")),
        dev: false,
        secure: false,
        allow_dirs: Vec::new(),
        allow_hosts: Vec::new(),
        api_workers: 2,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "--host" => o.host = val(),
            "--port" => o.port = val().parse().unwrap_or_else(|_| die("--port needs a number")),
            "--data" => o.data = PathBuf::from(val()),
            "--ui" => o.ui = PathBuf::from(val()),
            "--dev" => o.dev = true,
            "--secure" => o.secure = true,
            "--allow-dir" => o.allow_dirs.push(PathBuf::from(val())),
            "--allow-host" => o.allow_hosts.push(val()),
            "--api-workers" => o.api_workers = val().parse().ok().filter(|n| (1..=64).contains(n)).unwrap_or_else(|| die("--api-workers needs a number from 1 to 64")),
            "-V" | "--version" => {
                println!("quire-server {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-h" | "--help" => {
                println!("quire-server {} [--port 8766] [--data DIR] [--ui DIR] [--host 127.0.0.1] [--allow-dir DIR]… [--allow-host HOST]… [--api-workers N] [--secure] [--dev] [--version]", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => die(&format!("unknown option {a} (see --help)")),
        }
    }
    o
}

fn die(msg: &str) -> ! {
    eprintln!("quire-server: {msg}");
    std::process::exit(2)
}

fn main() {
    let o = opts();
    std::fs::create_dir_all(&o.data).unwrap_or_else(|e| die(&format!("can't make {}: {e}", o.data.display())));
    if !o.ui.join("index.html").exists() {
        die(&format!("no index.html in {} (use --ui)", o.ui.display()));
    }
    // secrets sealed in the data folder; the master key from the environment (or made once)
    let (master, in_folder) = quire::secrets::master_key(&o.data).unwrap_or_else(|e| die(&e));
    quire::secrets::use_file(&o.data.join("secrets.json"), &master).unwrap_or_else(|e| die(&e));
    if in_folder {
        eprintln!("quire-server: note — the master key is in {} (set QUIRE_SECRET_KEY to keep it out of the data folder)", o.data.join("master.key").display());
    }
    // data only from the data folder and the folders allowed
    let mut dirs = vec![o.data.clone()];
    dirs.extend(o.allow_dirs.iter().cloned());
    quire::fence_data(&dirs, (!o.allow_hosts.is_empty()).then(|| o.allow_hosts.clone()));
    // Python runs outside the fence: only when an administrator switches it on
    quire::guard_python(&o.data.join("users.json"));
    quire::warm_up();
    #[cfg(feature = "render")]
    let api = Some(quire::ApiService::start(&o.data, &o.ui, o.api_workers));
    #[cfg(not(feature = "render"))]
    let api: Option<Arc<Api>> = None;
    let sessions = Arc::new(Sessions { data: o.data.clone(), map: Mutex::new(HashMap::new()), swept: Mutex::new(Instant::now()) });
    let listener = TcpListener::bind((o.host.as_str(), o.port)).unwrap_or_else(|e| die(&format!("can't listen on {}:{}: {e}", o.host, o.port)));
    println!("Quire on http://{}:{}/  (data in {})", o.host, o.port, o.data.display());
    if o.host != "127.0.0.1" && o.host != "localhost" {
        eprintln!("quire-server: note — plain HTTP: put it behind HTTPS (a reverse proxy) before people sign in over a network");
    }
    let (ui, dev, secure) = (Arc::new(o.ui), o.dev, o.secure);
    let busy = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(TIMEOUT));
        let _ = stream.set_write_timeout(Some(TIMEOUT));
        use std::sync::atomic::Ordering::SeqCst;
        if busy.fetch_add(1, SeqCst) >= MAX_CONNECTIONS {
            busy.fetch_sub(1, SeqCst);
            let _ = reply(&mut stream, "503 Service Unavailable", "text/plain", b"busy", dev, "");
            continue;
        }
        let (sessions, ui, busy, api) = (sessions.clone(), ui.clone(), busy.clone(), api.clone());
        std::thread::spawn(move || {
            if let Err(e) = serve(stream, &sessions, &ui, api.as_deref(), dev, secure) {
                if e.kind() != std::io::ErrorKind::WouldBlock && e.kind() != std::io::ErrorKind::TimedOut {
                    eprintln!("quire-server: {e}");
                }
            }
            busy.fetch_sub(1, SeqCst);
        });
    }
}

/// The session cookie of a request's `Cookie:` header.
fn cookie(header: &str) -> Option<String> {
    header.split(';').filter_map(|c| c.trim().split_once('=')).find(|(k, _)| *k == COOKIE).map(|(_, v)| v.trim().to_string()).filter(|v| v.chars().all(|c| c.is_ascii_alphanumeric()) && !v.is_empty())
}

/// One request: a command, or a file of the UI.
fn serve(stream: TcpStream, sessions: &Sessions, ui: &Path, api: Option<&Api>, dev: bool, secure: bool) -> std::io::Result<()> {
    // the request line and headers: 64 KB at most
    let mut reader = BufReader::new(stream.try_clone()?.take(64 * 1024));
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let mut parts = first.split_whitespace();
    let (method, target) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("/").to_string());
    let (mut len, mut sid) = (0usize, None);
    // the report API's headers (its key, a retried request's key)
    let mut api_headers: Vec<(String, String)> = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().unwrap_or(0);
            } else if k.trim().eq_ignore_ascii_case("cookie") {
                sid = sid.or_else(|| cookie(v));
            } else if k.trim().eq_ignore_ascii_case("authorization") || k.trim().eq_ignore_ascii_case("idempotency-key") {
                api_headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    let mut out = stream;
    if len > MAX_BODY {
        return reply(&mut out, "413 Payload Too Large", "text/plain", b"too large", dev, "");
    }
    // the body, past the header limit
    reader.get_mut().set_limit(len as u64);
    let path = target.split(['?', '#']).next().unwrap_or("/");
    match (method.as_str(), path) {
        ("OPTIONS", _) => reply(&mut out, "204 No Content", "text/plain", b"", dev, ""),
        (_, p) if p.starts_with("/api/v1/") || p == "/api/v1" => {
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body)?;
            api_reply(&mut out, api, &method, path, &api_headers, &body, dev)
        }
        ("POST", "/api/call") => {
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body)?;
            // pages from another port (dev checks) can't carry the cookie: they share one workspace
            if sid.is_none() && dev {
                sid = Some("dev".into());
            }
            let (core, new) = sessions.get(sid.as_deref());
            let req: Json = serde_json::from_slice(&body).unwrap_or(Json::Null);
            let cmd = req["cmd"].as_str().unwrap_or("");
            let args = if req["args"].is_null() { json!({}) } else { req["args"].clone() };
            let (status, body) = match quire::dispatch(&core, cmd, &args) {
                Ok(v) => ("200 OK", v.to_string()),
                Err(e) => ("500 Internal Server Error", json!(e).to_string()),
            };
            let set = new.filter(|id| id != "dev").map(|id| format!("Set-Cookie: {COOKIE}={id}; Path=/; HttpOnly; SameSite=Strict{}\r\n", if secure { "; Secure" } else { "" })).unwrap_or_default();
            reply(&mut out, status, "application/json", body.as_bytes(), dev, &set)
        }
        ("GET" | "HEAD", _) => match ui_file(ui, path) {
            Some(f) => match std::fs::read(&f) {
                Ok(bytes) => reply(&mut out, "200 OK", mime(&f), if method == "HEAD" { b"" } else { &bytes }, dev, ""),
                Err(_) => reply(&mut out, "404 Not Found", "text/plain", b"not found", dev, ""),
            },
            None => reply(&mut out, "404 Not Found", "text/plain", b"not found", dev, ""),
        },
        _ => reply(&mut out, "405 Method Not Allowed", "text/plain", b"", dev, ""),
    }
}

/// A request to the report API, answered by it (or refused: built without the renderer).
#[cfg(feature = "render")]
fn api_reply(out: &mut TcpStream, api: Option<&Api>, method: &str, path: &str, headers: &[(String, String)], body: &[u8], dev: bool) -> std::io::Result<()> {
    let Some(api) = api else { return reply(out, "503 Service Unavailable", "application/json", br#"{"error":{"status":503,"message":"The report API isn't running."}}"#, dev, "") };
    let r = api.handle(method, path, headers, body);
    let status = format!("{} {}", r.status, reason(r.status));
    let extra: String = r.headers.iter().map(|(k, v)| format!("{k}: {}\r\n", v.replace(['\r', '\n'], ""))).collect();
    reply(out, &status, r.kind, if method == "HEAD" { b"" } else { &r.body }, dev, &extra)
}

#[cfg(not(feature = "render"))]
fn api_reply(out: &mut TcpStream, _api: Option<&Api>, _method: &str, _path: &str, _headers: &[(String, String)], _body: &[u8], dev: bool) -> std::io::Result<()> {
    reply(out, "501 Not Implemented", "application/json", br#"{"error":{"status":501,"message":"This server was built without the report renderer (cargo build --features render)."}}"#, dev, "")
}

#[cfg(feature = "render")]
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        _ => "Internal Server Error",
    }
}

fn reply(out: &mut TcpStream, status: &str, kind: &str, body: &[u8], dev: bool, extra: &str) -> std::io::Result<()> {
    let cors = if dev { "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type\r\n" } else { "" };
    // the UI changes with the server: never a stale module
    // no framing by other sites, no guessing of types, no referrer to other sites
    let safe = "X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: frame-ancestors 'none'\r\nReferrer-Policy: same-origin\r\n";
    write!(out, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{safe}{cors}{extra}Connection: close\r\n\r\n", body.len())?;
    out.write_all(body)?;
    out.flush()
}

/// The UI file a path names, never outside the UI folder.
fn ui_file(ui: &Path, path: &str) -> Option<PathBuf> {
    let path = percent_decode(path);
    let rel = Path::new(path.trim_start_matches('/'));
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let f = if rel.as_os_str().is_empty() { ui.join("index.html") } else { ui.join(rel) };
    f.is_file().then_some(f)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "md" | "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
