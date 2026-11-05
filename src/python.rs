//! Python data sources: a persistent `python3` worker process spoken to over
//! newline-delimited JSON on stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const WORKER: &str = include_str!("worker.py");

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PyResult {
    pub ok: bool,
    pub value: Json,
    pub repr: Option<String>,
    pub stdout: String,
    pub error: Option<String>,
    pub ms: f64,
    /// Process-level failure (not a Python exception) — never cached.
    #[serde(skip)]
    pub fatal: bool,
}

impl PyResult {
    pub fn failure(msg: impl Into<String>) -> Self {
        PyResult { ok: false, error: Some(msg.into()), fatal: true, ..Default::default() }
    }
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Default)]
pub struct PyEngine {
    proc: Option<Proc>,
    cache: HashMap<String, PyResult>,
    python: Option<String>,
    info: Option<Json>,
}

fn works(cmd: &str) -> bool {
    Command::new(cmd)
        .args(["-c", "import sys; sys.exit(0 if sys.version_info >= (3, 8) else 1)"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn find_python() -> Option<String> {
    let mut cands: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("QUIRE_PYTHON") {
        cands.push(p);
    }
    cands.push("python3".into());
    // GUI apps launched from Finder don't inherit the shell PATH (pyenv, conda…)
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    for flags in ["-lc", "-ilc"] {
        if let Ok(out) = Command::new(&shell).args([flags, "command -v python3"]).stdin(Stdio::null()).output() {
            if let Some(line) = String::from_utf8_lossy(&out.stdout).lines().rev().find(|l| l.starts_with('/')) {
                cands.push(line.trim().to_string());
            }
        }
    }
    for p in ["/opt/homebrew/bin/python3", "/usr/local/bin/python3", "/usr/bin/python3"] {
        cands.push(p.into());
    }
    cands.into_iter().find(|c| works(c))
}

impl PyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure(&mut self) -> Result<(), String> {
        check()?;
        if let Some(p) = &mut self.proc {
            if matches!(p.child.try_wait(), Ok(None)) {
                return Ok(());
            }
            self.proc = None;
        }
        let py = match &self.python {
            Some(p) => p.clone(),
            None => {
                let p = find_python().ok_or(
                    "Python 3 was not found. Install Python 3 (python.org or Homebrew), or set QUIRE_PYTHON to its path.",
                )?;
                self.python = Some(p.clone());
                p
            }
        };
        let script = std::env::temp_dir().join(format!("quire_worker_{}.py", std::process::id()));
        std::fs::write(&script, WORKER).map_err(|e| format!("Could not write Python worker: {e}"))?;
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        let mut child = Command::new(&py)
            .arg("-u")
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PYTHONIOENCODING", "utf-8")
            .env("MPLBACKEND", "Agg")
            .current_dir(home)
            .spawn()
            .map_err(|e| format!("Could not start Python ({py}): {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let errbuf = Arc::new(Mutex::new(VecDeque::new()));
        let eb = errbuf.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut b = eb.lock().unwrap();
                b.push_back(line);
                if b.len() > 40 {
                    b.pop_front();
                }
            }
        });
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(line) => {
                let v: Json = serde_json::from_str(&line).unwrap_or(json!({}));
                let mut info = v;
                info["executable"] = json!(py);
                self.info = Some(info);
            }
            Err(_) => {
                let _ = child.kill();
                let tail: Vec<String> = errbuf.lock().unwrap().iter().cloned().collect();
                return Err(format!("Python failed to start.\n{}", tail.join("\n")));
            }
        }
        self.proc = Some(Proc { child, stdin, rx, stderr: errbuf });
        Ok(())
    }

    fn request(&mut self, req: Json, timeout: Duration) -> PyResult {
        if let Err(e) = self.ensure() {
            return PyResult::failure(e);
        }
        let p = self.proc.as_mut().unwrap();
        if writeln!(p.stdin, "{}", req).and_then(|_| p.stdin.flush()).is_err() {
            self.proc = None;
            return PyResult::failure("The Python process exited unexpectedly.");
        }
        match p.rx.recv_timeout(timeout) {
            Ok(line) => serde_json::from_str::<PyResult>(&line)
                .unwrap_or_else(|e| PyResult::failure(format!("Invalid response from Python: {e}"))),
            Err(RecvTimeoutError::Timeout) => {
                let _ = p.child.kill();
                self.proc = None;
                PyResult::failure(format!(
                    "Python code ran longer than {}s and was stopped. The runtime has been restarted.",
                    timeout.as_secs()
                ))
            }
            Err(RecvTimeoutError::Disconnected) => {
                let tail: Vec<String> = p.stderr.lock().unwrap().iter().cloned().collect();
                self.proc = None;
                PyResult::failure(format!("The Python process crashed.\n{}", tail.join("\n")))
            }
        }
    }

    /// Runs a dataset script. Results are memoised on (code, params) until refresh.
    pub fn dataset(&mut self, code: &str, params: &Json, cwd: Option<&str>, sources: Option<&Json>) -> PyResult {
        let src = sources.cloned().unwrap_or(Json::Null);
        let key = format!("{code}\u{0}{params}\u{0}{}\u{0}{src}", cwd.unwrap_or(""));
        if let Some(r) = self.cache.get(&key) {
            return r.clone();
        }
        let res = self.request(json!({"kind": "dataset", "code": code, "params": params, "cwd": cwd, "sources": src}), Duration::from_secs(120));
        if res.ok {
            if self.cache.len() > 200 {
                self.cache.clear();
            }
            self.cache.insert(key, res.clone());
        }
        res
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Runs a script on a table (the SQL workspace's last result, as `df`).
    pub fn run_on(&mut self, code: &str, data: &Json, params: &Json) -> PyResult {
        self.request(json!({"kind": "run", "code": code, "data": data, "params": params, "cwd": null}), Duration::from_secs(120))
    }

    /// Uses this interpreter from now on (None: find one). The running worker stops.
    pub fn set_python(&mut self, path: Option<String>) {
        let path = path.filter(|p| !p.trim().is_empty());
        if self.python == path && (path.is_some() || self.proc.is_some()) {
            return;
        }
        if let Some(p) = &mut self.proc {
            let _ = p.child.kill();
        }
        self.proc = None;
        self.python = path;
        self.info = None;
        self.cache.clear();
    }

    pub fn info(&mut self) -> Json {
        match self.ensure() {
            Ok(()) => self.info.clone().unwrap_or(json!({})),
            Err(e) => json!({ "error": e }),
        }
    }
}

impl Drop for PyEngine {
    fn drop(&mut self) {
        if let Some(mut p) = self.proc.take() {
            let _ = p.child.kill();
        }
    }
}

// ------------------------------------------------------------------ the server's switch

/// On a shared server, Python runs as the server's user, outside the data fence: it runs only when an
/// administrator switches it on (Admin → Access). The desktop app is never guarded.
static GUARDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static ALLOWED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Guards Python from now on (the web server, at start), switched on or off as saved.
pub fn guard(allowed: bool) {
    ALLOWED.store(allowed, std::sync::atomic::Ordering::SeqCst);
    GUARDED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Switches Python on or off (where it is guarded).
pub fn allow(on: bool) {
    ALLOWED.store(on, std::sync::atomic::Ordering::SeqCst);
}

pub fn guarded() -> bool {
    GUARDED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Whether Python may run now.
pub fn check() -> Result<(), String> {
    if guarded() && !ALLOWED.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Python is switched off on this server. An administrator can switch it on in Admin → Access.".into());
    }
    Ok(())
}

/// One Python on this computer: where, which version, whether pandas is there.
pub fn probe(path: &str) -> Option<Json> {
    let out = Command::new(path)
        .args(["-c", "import sys, importlib.util as u; print(sys.version.split()[0]); print(u.find_spec('pandas') is not None); print(sys.prefix)"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut l = text.lines();
    let version = l.next()?.trim().to_string();
    let pandas = l.next().map(|x| x.trim() == "True").unwrap_or(false);
    let prefix = l.next().unwrap_or("").trim().to_string();
    if !version.starts_with('3') {
        return None;
    }
    Some(json!({"path": path, "version": version, "pandas": pandas, "prefix": prefix}))
}

/// The Pythons found on this computer: on the PATH, Homebrew and the system's,
/// pyenv versions, conda / mamba environments and virtualenvs.
pub fn environments() -> Vec<Json> {
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let mut cands: Vec<(String, String)> = Vec::new(); // (path, kind)
    if let Ok(p) = std::env::var("QUIRE_PYTHON") {
        cands.push((p, "QUIRE_PYTHON".into()));
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    if let Ok(out) = Command::new(&shell).args(["-lc", "which -a python3 python 2>/dev/null"]).stdin(Stdio::null()).output() {
        for l in String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.starts_with('/')) {
            cands.push((l.trim().to_string(), "PATH".into()));
        }
    }
    for p in ["/opt/homebrew/bin/python3", "/usr/local/bin/python3", "/usr/bin/python3"] {
        cands.push((p.into(), if p.starts_with("/usr/bin") { "system".into() } else { "Homebrew".into() }));
    }
    let each = |dir: std::path::PathBuf, kind: &str, out: &mut Vec<(String, String)>| {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            v.sort();
            for d in v {
                for b in ["bin/python3", "bin/python"] {
                    let p = d.join(b);
                    if p.exists() {
                        out.push((p.display().to_string(), kind.into()));
                        break;
                    }
                }
            }
        }
    };
    each(home.join(".pyenv/versions"), "pyenv", &mut cands);
    for base in ["miniconda3", "anaconda3", "miniforge3", "mambaforge", "micromamba", "opt/miniconda3", "opt/anaconda3"] {
        let b = home.join(base);
        let py = b.join("bin/python3");
        if py.exists() {
            cands.push((py.display().to_string(), "conda (base)".into()));
        }
        each(b.join("envs"), "conda", &mut cands);
    }
    each(home.join(".virtualenvs"), "virtualenv", &mut cands);
    each(home.join(".local/share/virtualenvs"), "pipenv", &mut cands);
    let venv = home.join(".venv/bin/python3");
    if venv.exists() {
        cands.push((venv.display().to_string(), "venv".into()));
    }
    // one entry per interpreter (symlinks to the same one are the same)
    let mut seen = std::collections::HashSet::new();
    let mut prefixes = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (p, kind) in cands {
        let real = std::fs::canonicalize(&p).map(|x| x.display().to_string()).unwrap_or(p.clone());
        if !seen.insert(real) {
            continue;
        }
        if let Some(mut j) = probe(&p) {
            // shims and links to one environment are that environment once
            if !prefixes.insert(j["prefix"].as_str().unwrap_or("").to_string()) {
                continue;
            }
            j["kind"] = json!(kind);
            out.push(j);
        }
    }
    out
}
