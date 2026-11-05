//! Secrets for the web server and the desktop app outside macOS: AI keys and database passwords, kept in one file sealed with a
//! master key (ChaCha20-Poly1305), instead of the macOS Keychain the Mac app uses.
//!
//! The master key comes from `QUIRE_SECRET_KEY` (any passphrase; keep it outside the data
//! folder: then a copy of the folder alone doesn't give the secrets away). Without it, the server
//! makes `master.key` next to the secrets the first time and says so.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

struct Store {
    path: PathBuf,
    cipher: ChaCha20Poly1305,
    /// One change at a time (read, change, write the whole file).
    lock: Mutex<()>,
}

static STORE: OnceLock<Store> = OnceLock::new();

/// Keeps secrets in `path` from now on (the web server). Returns where the master key came from.
pub fn use_file(path: &Path, master: &str) -> Result<(), String> {
    let key = Sha256::digest(master.as_bytes());
    let s = Store { path: path.to_path_buf(), cipher: ChaCha20Poly1305::new(Key::from_slice(&key)), lock: Mutex::new(()) };
    // a master key that doesn't open the file already there is a mistake, not a fresh start
    let all = read(&s.path)?;
    if let Some(v) = all.values().next() {
        open(&s.cipher, v).map_err(|_| format!("QUIRE_SECRET_KEY doesn't open {} (a different master key?)", path.display()))?;
    }
    STORE.set(s).map_err(|_| "The secrets file is set already.".to_string())
}

/// The master key: `QUIRE_SECRET_KEY`, else `master.key` in `dir` (made the first time). The
/// second item says whether it came from the folder.
pub fn master_key(dir: &Path) -> Result<(String, bool), String> {
    if let Ok(k) = std::env::var("QUIRE_SECRET_KEY") {
        if k.len() < 16 {
            return Err("QUIRE_SECRET_KEY must be at least 16 characters.".into());
        }
        return Ok((k, false));
    }
    let f = dir.join("master.key");
    if let Ok(k) = std::fs::read_to_string(&f) {
        return Ok((k.trim().to_string(), true));
    }
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).map_err(|e| e.to_string())?;
    let k: String = b.iter().map(|x| format!("{x:02x}")).collect();
    write_private(&f, k.as_bytes())?;
    Ok((k, true))
}

/// Whether secrets are kept here (the web server) rather than in the Keychain.
pub fn enabled() -> bool {
    STORE.get().is_some()
}

pub fn get(name: &str) -> Option<String> {
    let s = STORE.get()?;
    let _g = s.lock.lock().unwrap_or_else(|e| e.into_inner());
    read(&s.path).ok()?.get(name).and_then(|v| open(&s.cipher, v).ok())
}

pub fn has(name: &str) -> bool {
    let Some(s) = STORE.get() else { return false };
    let _g = s.lock.lock().unwrap_or_else(|e| e.into_inner());
    read(&s.path).map(|m| m.contains_key(name)).unwrap_or(false)
}

/// Saves a secret, or removes it with None.
pub fn set(name: &str, value: Option<&str>) -> Result<(), String> {
    let s = STORE.get().ok_or("No secrets file.")?;
    let _g = s.lock.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read(&s.path)?;
    match value {
        Some(v) => {
            all.insert(name.to_string(), seal(&s.cipher, v)?);
        }
        None => {
            all.remove(name);
        }
    }
    write_private(&s.path, serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?.as_bytes())
}

fn read(path: &Path) -> Result<BTreeMap<String, String>, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{} is damaged: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(format!("Could not read {}: {e}", path.display())),
    }
}

/// nonce ‖ sealed text, as hex.
fn seal(c: &ChaCha20Poly1305, v: &str) -> Result<String, String> {
    let mut n = [0u8; 12];
    getrandom::getrandom(&mut n).map_err(|e| e.to_string())?;
    let sealed = c.encrypt(Nonce::from_slice(&n), v.as_bytes()).map_err(|_| "Could not seal the secret.".to_string())?;
    Ok(n.iter().chain(sealed.iter()).map(|x| format!("{x:02x}")).collect())
}

fn open(c: &ChaCha20Poly1305, hex: &str) -> Result<String, ()> {
    let b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect::<Option<_>>().ok_or(())?;
    if b.len() < 12 {
        return Err(());
    }
    let plain = c.decrypt(Nonce::from_slice(&b[..12]), &b[12..]).map_err(|_| ())?;
    String::from_utf8(plain).map_err(|_| ())
}

/// Written whole (a temporary file renamed over it), readable by this user only.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_sealed_and_come_back() {
        let c = ChaCha20Poly1305::new(Key::from_slice(&Sha256::digest(b"a master key for the test")));
        let s = seal(&c, "sk-ant-secret").unwrap();
        assert!(!s.contains("secret"));
        assert_eq!(open(&c, &s).unwrap(), "sk-ant-secret");
        // another master key doesn't open it
        let other = ChaCha20Poly1305::new(Key::from_slice(&Sha256::digest(b"another master key here")));
        assert!(open(&other, &s).is_err());
    }
}
