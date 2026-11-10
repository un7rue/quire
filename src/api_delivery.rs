//! Delivering the report API's files besides the download (backlog/report-api.md): copies to a
//! destination Admin sets up — a folder (a mounted file share too) or an S3-compatible bucket — and
//! the callback a request asks for when its job is done.
//!
//! Requests go out through the system's `curl`, as the AI providers' do: its settings (the S3 keys
//! included) travel on its stdin, never on a command line. S3 uploads are signed by curl
//! (`--aws-sigv4`, Signature Version 4), so any S3-compatible service works (AWS, MinIO, Yandex
//! Object Storage…).
//!
//! **Callbacks** are signed: `X-Quire-Signature: sha256=<hex>` is HMAC-SHA256 of
//! `<X-Quire-Timestamp>.<body>` with the account's signing key, which is the SHA-256 (hex) of its
//! API key — the caller works it out from the key it holds, and checks the time is recent.
// callbacks and file names are the report API's (src/api_jobs.rs, built with the renderer); Admin's
// *Test* uses the copying in every build
#![cfg_attr(not(feature = "render"), allow(dead_code))]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::api_accounts::ApiTarget;

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let pad = |x: u8| k.iter().map(|b| b ^ x).collect::<Vec<u8>>();
    let inner = Sha256::new().chain_update(pad(0x36)).chain_update(msg).finalize();
    Sha256::new().chain_update(pad(0x5c)).chain_update(inner).finalize().into()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The signature of a callback's body sent at `timestamp` (seconds since 1970).
pub fn signature(sign_key: &str, timestamp: i64, body: &str) -> String {
    format!("sha256={}", hex(&hmac_sha256(sign_key.as_bytes(), format!("{timestamp}.{body}").as_bytes())))
}

/// A value for curl's config syntax.
fn cq(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

/// One request through curl; its settings on stdin. Returns (status, body).
fn curl(cfg: &str) -> Result<(u16, String), String> {
    let mut child = Command::new("curl").args(["-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("curl is not available: {e}"))?;
    child.stdin.take().ok_or("curl stdin")?.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    match code.trim().parse::<u16>() {
        Ok(c) if c > 0 => Ok((c, body.to_string())),
        _ => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if err.is_empty() { "No answer.".into() } else { err })
        }
    }
}

/// An S3 object key, each part percent-encoded (letters, digits and `-._~` stay).
fn s3_path(key: &str) -> String {
    key.split('/').map(|part| part.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>()).collect::<Vec<_>>().join("/")
}

/// A file name safe in folders and buckets alike: letters (any language), digits and ` .-_—()`.
pub fn safe_name(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_alphanumeric() || " .-_—()".contains(c) { c } else { '-' }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "report".into() } else { s.chars().take(150).collect() }
}

/// Puts `bytes` at `rel` (`a/b/c.pdf`) in the destination; returns where it went.
pub fn copy(t: &ApiTarget, rel: &str, bytes: &[u8]) -> Result<String, String> {
    if rel.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
        return Err(format!("Not a file's place: {rel}"));
    }
    match t.kind.as_str() {
        "folder" => {
            let path: PathBuf = Path::new(&t.folder).join(rel);
            if let Some(d) = path.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("Can't make {}: {e}", d.display()))?;
            }
            std::fs::write(&path, bytes).map_err(|e| format!("Can't write {}: {e}", path.display()))?;
            Ok(path.display().to_string())
        }
        "s3" => {
            let secret = t.secret().ok_or("The destination has no secret key.")?;
            let key = if t.prefix.is_empty() { rel.to_string() } else { format!("{}/{rel}", t.prefix) };
            let url = format!("{}/{}/{}", t.endpoint, t.bucket, s3_path(&key));
            let tmp = std::env::temp_dir().join(format!("quire-s3-{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
            std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
            let cfg = format!(
                "url = {}\nupload-file = {}\naws-sigv4 = {}\nuser = {}\nheader = \"Content-Type: {}\"\nsilent\nshow-error\nmax-time = 300\nwrite-out = \"\\n%{{http_code}}\"\n",
                cq(&url), cq(&tmp.display().to_string()), cq(&format!("aws:amz:{}:s3", t.region)), cq(&format!("{}:{secret}", t.access_key)),
                if rel.ends_with(".pdf") { "application/pdf" } else { "text/plain" });
            let r = curl(&cfg);
            let _ = std::fs::remove_file(&tmp);
            let (code, body) = r.map_err(|e| format!("Couldn't reach {}: {e}", t.endpoint))?;
            if !(200..300).contains(&code) {
                let msg = body.split("<Message>").nth(1).and_then(|m| m.split("</Message>").next()).unwrap_or("").trim();
                return Err(format!("The bucket refused it ({code}){}", if msg.is_empty() { String::new() } else { format!(": {msg}") }));
            }
            Ok(format!("s3://{}/{key}", t.bucket))
        }
        k => Err(format!("Unknown kind of destination “{k}”.")),
    }
}

/// Writes a small file to the destination and takes it away again (S3: left, as a bucket may not
/// let its key delete): Admin's *Test*.
pub fn test(t: &ApiTarget) -> Result<String, String> {
    let rel = format!("quire-test-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let at = copy(t, &rel, b"Quire can write reports here.\n")?;
    if t.kind == "folder" {
        let _ = std::fs::remove_file(Path::new(&t.folder).join(&rel));
    }
    Ok(at)
}

/// POSTs a finished job to its callback, signed; returns the answer's status (2xx: delivered).
pub fn call_back(url: &str, sign_key: &str, job: &str, body: &str) -> Result<u16, String> {
    let ts = chrono::Utc::now().timestamp();
    let tmp = std::env::temp_dir().join(format!("quire-cb-{}-{}.json", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
    let mut cfg = format!("url = {}\nrequest = \"POST\"\nsilent\nshow-error\nmax-time = 30\nwrite-out = \"\\n%{{http_code}}\"\n", cq(url));
    for h in ["Content-Type: application/json".to_string(), "User-Agent: Quire".to_string(), "X-Quire-Event: job.finished".to_string(), format!("X-Quire-Job: {job}"),
        format!("X-Quire-Timestamp: {ts}"), format!("X-Quire-Signature: {}", signature(sign_key, ts, body))] {
        cfg.push_str(&format!("header = {}\n", cq(&h)));
    }
    cfg.push_str(&format!("data-binary = {}\n", cq(&format!("@{}", tmp.display()))));
    let r = curl(&cfg);
    let _ = std::fs::remove_file(&tmp);
    r.map(|(code, _)| code)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// A stand-in HTTP server: answers `status` to every request and keeps each (method, path,
    /// headers, body).
    pub(crate) fn stand_in(status: u16) -> (String, Arc<Mutex<Vec<(String, String, Vec<(String, String)>, Vec<u8>)>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let keep = seen.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut out = s.try_clone().unwrap();
                let mut r = BufReader::new(s);
                let mut first = String::new();
                if r.read_line(&mut first).is_err() { continue }
                let mut it = first.split_whitespace();
                let (m, p) = (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string());
                let mut headers = Vec::new();
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" { break }
                    if let Some((k, v)) = line.split_once(':') { headers.push((k.trim().to_lowercase(), v.trim().to_string())); }
                }
                let len: usize = headers.iter().find(|(k, _)| k == "content-length").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
                if headers.iter().any(|(k, v)| k == "expect" && v.eq_ignore_ascii_case("100-continue")) {
                    let _ = out.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
                }
                let mut body = vec![0u8; len];
                let _ = r.read_exact(&mut body);
                keep.lock().unwrap().push((m, p, headers, body));
                let _ = write!(out, "HTTP/1.1 {status} X\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
            }
        });
        (base, seen)
    }

    #[test]
    fn hmac_matches_rfc_4231() {
        // test case 2
        assert_eq!(hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        // test case 6: a key longer than a block
        assert_eq!(hex(&hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First")), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    }

    /// A folder copy lands in its subfolders; a path out of the folder is refused.
    #[test]
    fn copies_to_a_folder() {
        let dir = std::env::temp_dir().join(format!("quire_dest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t = ApiTarget { id: "share".into(), kind: "folder".into(), folder: dir.display().to_string(), ..Default::default() };
        let at = copy(&t, "crm/2026-11-10/j1/C0001 — Отчёт.pdf", b"%PDF-1.7").unwrap();
        assert_eq!(std::fs::read(&at).unwrap(), b"%PDF-1.7");
        assert!(copy(&t, "../escape.pdf", b"x").is_err() && copy(&t, "a//b.pdf", b"x").is_err());
        assert!(test(&t).is_ok());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "the test file is taken away");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An S3 copy is a signed PUT of the file to <endpoint>/<bucket>/<prefix>/<key>; a refusal says why.
    #[test]
    fn copies_to_s3_signed() {
        let (base, seen) = stand_in(200);
        let t = ApiTarget { id: "s3".into(), kind: "s3".into(), endpoint: base.clone(), bucket: "quire".into(), region: "eu-central-1".into(), prefix: "api".into(), access_key: "AKID".into(), secret_key: "SECRET".into(), ..Default::default() };
        let pdf = vec![b'%'; 300_000];
        let at = copy(&t, "crm/2026-11-10/j1/C0001.pdf", &pdf).unwrap();
        assert_eq!(at, "s3://quire/api/crm/2026-11-10/j1/C0001.pdf");
        let got = seen.lock().unwrap().pop().unwrap();
        assert_eq!((got.0.as_str(), got.1.as_str()), ("PUT", "/quire/api/crm/2026-11-10/j1/C0001.pdf"));
        let auth = got.2.iter().find(|(k, _)| k == "authorization").map(|(_, v)| v.clone()).unwrap_or_default();
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKID/") && auth.contains("/eu-central-1/s3/aws4_request"), "{auth}");
        assert!(!auth.contains("SECRET"));
        assert!(got.2.iter().any(|(k, _)| k == "x-amz-content-sha256"));
        assert_eq!(got.3, pdf);
        let (base, _) = stand_in(403);
        let t = ApiTarget { endpoint: base, ..t };
        assert!(copy(&t, "x.pdf", b"x").unwrap_err().contains("403"));
    }

    /// A callback is a POST of the job, signed over its timestamp and body.
    #[test]
    fn callbacks_are_signed() {
        let (base, seen) = stand_in(204);
        let body = r#"{"job_id":"j1","status":"done"}"#;
        let code = call_back(&format!("{base}/hook"), "k3y", "j1", body).unwrap();
        assert_eq!(code, 204);
        let got = seen.lock().unwrap().pop().unwrap();
        let h = |n: &str| got.2.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or_default();
        assert_eq!(got.0, "POST");
        assert_eq!(String::from_utf8(got.3.clone()).unwrap(), body);
        let ts: i64 = h("x-quire-timestamp").parse().unwrap();
        assert_eq!(h("x-quire-signature"), signature("k3y", ts, body));
        assert_eq!(h("x-quire-job"), "j1");
        // nobody there
        assert!(call_back("http://127.0.0.1:1/hook", "k", "j", "{}").is_err());
    }
}
