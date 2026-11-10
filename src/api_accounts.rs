//! The report API's side of Admin (backlog/report-api.md): the systems that may ask Quire for
//! reports, and what each published report accepts from them.
//!
//! **API accounts.** Each calling system (a CRM, a client portal, a scheduler) has an account of
//! its own: a key, the clients and products it may report on (the same access a person has) and
//! the published reports it may ask for. The key is shown once, when it is made; only a salted hash
//! is kept. A key reads `qk.<account>.<secret>`; a new one replaces the old at once.
//!
//! **What a report accepts.** Over the API a request always gives the client and the period; the
//! report's other parameters are taken only when Admin lets the API set them, and Admin can give
//! each a default for API requests (else the template's own applies). A report can be kept off the
//! API altogether.
//!
//! **Delivery.** Besides the download, an account's reports can be copied to a destination Admin
//! sets up (a folder or file share, or an S3-compatible bucket; src/api_delivery.rs). A request can
//! ask to be called back when its job is done, at an address on a host Admin allows for the account;
//! the call is signed with a key derived from the account's API key (`sign_key`), so the caller can
//! tell it came from Quire.
// signing in with a key is for the server's report API (src/api_jobs.rs, built with the renderer)
#![cfg_attr(not(feature = "render"), allow(dead_code))]

use std::collections::BTreeMap;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

use crate::auth::{Access, Roles, Session, Store};

/// How long finished files stay downloadable unless Admin says otherwise.
pub const RETENTION_DAYS: u32 = 7;
/// The start of every key, then the account and the secret.
const PREFIX: &str = "qk";

/// The API's settings, kept with the accounts (`users.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ApiSettings {
    pub clients: Vec<ApiClient>,
    /// Published report id → what it accepts over the API (a report not listed: client and period only).
    pub reports: BTreeMap<String, ApiReport>,
    /// Days finished files are kept for download.
    pub retention_days: u32,
    /// Where accounts' reports can be copied.
    pub targets: Vec<ApiTarget>,
}

impl Default for ApiSettings {
    fn default() -> Self {
        ApiSettings { clients: vec![], reports: BTreeMap::new(), retention_days: RETENTION_DAYS, targets: vec![] }
    }
}

/// A place reports are copied to: a folder (a mounted file share too), or an S3-compatible bucket.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct ApiTarget {
    pub id: String,
    pub name: String,
    /// "folder" or "s3"
    pub kind: String,
    /// The folder (an absolute path on the server).
    pub folder: String,
    /// S3: the service's address (`https://s3.eu-central-1.amazonaws.com`, a MinIO server…), the
    /// bucket, its region, a key prefix, and the access key; its secret is sealed in the server's
    /// secrets (`api.s3.<id>`), or kept here where there are none.
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    pub access_key: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub secret_key: String,
    pub by: String,
    pub at: String,
}

impl ApiTarget {
    fn secret_name(&self) -> String {
        format!("api.s3.{}", self.id)
    }

    /// The S3 secret key, wherever it is kept.
    pub fn secret(&self) -> Option<String> {
        if !self.secret_key.is_empty() {
            return Some(self.secret_key.clone());
        }
        crate::secrets::get(&self.secret_name())
    }

    /// As Admin sees it: no secret.
    pub fn public(&self) -> Json {
        json!({"id": self.id, "name": self.name, "kind": self.kind, "folder": self.folder, "endpoint": self.endpoint, "bucket": self.bucket,
            "region": self.region, "prefix": self.prefix, "access_key": self.access_key, "has_secret": self.secret().is_some()})
    }

    /// Where it puts things, in a few words.
    pub fn place(&self) -> String {
        if self.kind == "s3" { format!("s3://{}/{}", self.bucket, self.prefix.trim_matches('/')) } else { self.folder.clone() }
    }
}

/// What Admin sends to add or change a destination; an empty secret keeps the one saved.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ApiTargetEdit {
    pub was: String,
    pub name: String,
    pub kind: String,
    pub folder: String,
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    pub access_key: String,
    pub secret_key: String,
}

/// A system that asks for reports.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct ApiClient {
    /// Letters, digits and `-`: part of its key and of the log.
    pub id: String,
    pub name: String,
    pub description: String,
    /// Off: its key is refused (kept, so it can be switched back on).
    pub active: bool,
    pub access: Access,
    /// Published reports it may ask for (ids); empty = every published report.
    pub reports: Vec<String>,
    pub key_salt: String,
    pub key_hash: String,
    /// The key's last four characters, to tell keys apart.
    pub key_hint: String,
    pub key_at: String,
    /// Signs its callbacks: the SHA-256 of its key (hex), which its team can work out from the key.
    pub sign_key: String,
    /// The destination its reports are copied to ("" = only downloaded).
    pub copy_to: String,
    /// Hosts its callbacks may go to (none: no callbacks).
    pub callback_hosts: Vec<String>,
    pub by: String,
    pub at: String,
}

/// What a report accepts over the API.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct ApiReport {
    /// Kept off the API.
    pub off: bool,
    /// Parameter name → whether a request may set it, and its default for API requests.
    pub params: BTreeMap<String, ApiParam>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct ApiParam {
    /// A request may set it.
    pub accept: bool,
    /// Used when a request leaves it out: a text, a number or a list (null: the template's own).
    pub default: Json,
}

/// What Admin sends to add or change an account (the key is made apart).
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ApiClientEdit {
    /// The account changed ("" = a new one).
    pub was: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub access: Access,
    pub reports: Vec<String>,
    pub copy_to: String,
    pub callback_hosts: Vec<String>,
}

fn hash(salt: &str, secret: &str) -> String {
    // a key is 256 random bits: one round is enough (passwords are stretched, keys needn't be)
    Sha256::digest(format!("{salt}\u{0}{secret}").as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn random(n: usize) -> Result<Vec<u8>, String> {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).map_err(|e| format!("No randomness for a key: {e}"))?;
    Ok(b)
}

/// The same bytes, compared in the same time whatever they are.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl ApiClient {
    /// A new key for this account (the old one stops working); the key is returned once.
    pub fn new_key(&mut self) -> Result<String, String> {
        let secret = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random(32)?);
        self.key_salt = random(16)?.iter().map(|b| format!("{b:02x}")).collect();
        self.key_hash = hash(&self.key_salt, &secret);
        self.key_hint = secret.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        self.key_at = chrono::Local::now().to_rfc3339();
        let key = format!("{PREFIX}.{}.{secret}", self.id);
        self.sign_key = sign_key_of(&key);
        Ok(key)
    }

    /// The account as Admin sees it: no hash or salt.
    pub fn public(&self) -> Json {
        json!({
            "id": self.id, "name": self.name, "description": self.description, "active": self.active,
            "access": self.access, "reports": self.reports, "copy_to": self.copy_to, "callback_hosts": self.callback_hosts,
            "key": if self.key_hash.is_empty() { Json::Null } else { json!({"hint": self.key_hint, "at": self.key_at, "signs": !self.sign_key.is_empty()}) },
            "by": self.by, "at": self.at,
        })
    }

    /// Who the account acts as: a report user with the account's access.
    pub fn session(&self) -> Session {
        Session { login: format!("api:{}", self.id), name: self.name.clone(), roles: Roles { user: true, ..Default::default() }, access: self.access.clone() }
    }

    /// Whether it may ask for the published report `id`.
    pub fn may_use(&self, id: &str) -> bool {
        self.reports.is_empty() || self.reports.iter().any(|r| r == id)
    }
}

/// The key callbacks are signed with, from an API key: SHA-256 of the whole key, in hex.
pub fn sign_key_of(key: &str) -> String {
    Sha256::digest(key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// A host as allowed for callbacks: lowercase, with its port if one is given.
fn host_ok(h: &str) -> Result<String, String> {
    let h = h.trim().to_lowercase();
    if h.is_empty() || !h.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c)) {
        return Err(format!("“{h}” isn't a host name (like crm.bank.local or crm.bank.local:8443)."));
    }
    Ok(h)
}

/// The account a key belongs to, and the session it acts with.
pub fn sign_in(s: &Store, key: &str) -> Result<(ApiClient, Session), String> {
    const BAD: &str = "Invalid API key.";
    let mut parts = key.trim().splitn(3, '.');
    let (Some(PREFIX), Some(id), Some(secret)) = (parts.next(), parts.next(), parts.next()) else { return Err(BAD.into()) };
    let c = s.api.clients.iter().find(|c| c.id == id && !c.key_hash.is_empty()).ok_or(BAD)?;
    if !same(&hash(&c.key_salt, secret), &c.key_hash) {
        return Err(BAD.into());
    }
    if !c.active {
        return Err(format!("The API account “{}” is switched off.", c.name));
    }
    Ok((c.clone(), c.session()))
}

/// The id a name makes: lowercase letters, digits and `-`.
fn slug(s: &str) -> String {
    let s: String = s.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    s.split('-').filter(|x| !x.is_empty()).collect::<Vec<_>>().join("-").chars().take(40).collect()
}

/// Adds or changes an account. A new account gets its key, returned once.
pub fn save(store: &mut Store, e: ApiClientEdit, me: &str) -> Result<Option<String>, String> {
    let name = e.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the API account a name.".into());
    }
    let id = if e.id.trim().is_empty() { slug(&name) } else { e.id.trim().to_string() };
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("An API account's id is lowercase letters, digits and - only.".into());
    }
    let at = (!e.was.is_empty()).then(|| store.api.clients.iter().position(|c| c.id == e.was)).flatten();
    if !e.was.is_empty() && at.is_none() {
        return Err(format!("There is no API account “{}”.", e.was));
    }
    if store.api.clients.iter().enumerate().any(|(i, c)| c.id == id && Some(i) != at) {
        return Err(format!("There is already an API account “{id}”."));
    }
    if let Some(r) = e.reports.iter().find(|r| !store.reports.iter().any(|p| &p.id == *r)) {
        return Err(format!("There is no published report “{r}”."));
    }
    if !e.copy_to.is_empty() && !store.api.targets.iter().any(|t| t.id == e.copy_to) {
        return Err(format!("There is no destination “{}”.", e.copy_to));
    }
    let mut hosts = Vec::new();
    for h in e.callback_hosts.iter().filter(|h| !h.trim().is_empty()) {
        let h = host_ok(h)?;
        if !hosts.contains(&h) {
            hosts.push(h);
        }
    }
    let mut c = at.map(|i| store.api.clients[i].clone()).unwrap_or_else(|| ApiClient { by: me.into(), at: chrono::Local::now().to_rfc3339(), ..Default::default() });
    // the key carries the id: a renamed account needs a new key
    let renamed = at.is_some() && c.id != id;
    c.id = id;
    c.name = name;
    c.description = e.description.trim().to_string();
    c.active = e.active;
    c.access = e.access;
    c.reports = e.reports;
    c.copy_to = e.copy_to;
    c.callback_hosts = hosts;
    let key = if at.is_none() || renamed { Some(c.new_key()?) } else { None };
    match at {
        Some(i) => store.api.clients[i] = c,
        None => store.api.clients.push(c),
    }
    Ok(key)
}

/// A new key for an account; the old one stops working.
pub fn new_key(store: &mut Store, id: &str) -> Result<String, String> {
    store.api.clients.iter_mut().find(|c| c.id == id).ok_or_else(|| format!("There is no API account “{id}”."))?.new_key()
}

pub fn remove(store: &mut Store, id: &str) -> Result<(), String> {
    let n = store.api.clients.len();
    store.api.clients.retain(|c| c.id != id);
    if store.api.clients.len() == n {
        return Err(format!("There is no API account “{id}”."));
    }
    Ok(())
}

/// Adds or changes a destination. On the server an S3 secret is sealed in the secrets file.
pub fn save_target(store: &mut Store, e: ApiTargetEdit, me: &str) -> Result<String, String> {
    let name = e.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the destination a name.".into());
    }
    let at = (!e.was.is_empty()).then(|| store.api.targets.iter().position(|t| t.id == e.was)).flatten();
    if !e.was.is_empty() && at.is_none() {
        return Err(format!("There is no destination “{}”.", e.was));
    }
    let mut t = at.map(|i| store.api.targets[i].clone()).unwrap_or_else(|| ApiTarget { by: me.into(), at: chrono::Local::now().to_rfc3339(), ..Default::default() });
    if t.id.is_empty() {
        let base = slug(&name);
        let base = if base.is_empty() { "destination".to_string() } else { base };
        t.id = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|id| !store.api.targets.iter().any(|x| &x.id == id)).unwrap();
    }
    t.name = name;
    t.kind = e.kind.trim().to_string();
    match t.kind.as_str() {
        "folder" => {
            let f = e.folder.trim();
            if !std::path::Path::new(f).is_absolute() {
                return Err("Give the folder as a full path on the server (like /mnt/reports).".into());
            }
            t.folder = f.to_string();
            (t.endpoint, t.bucket, t.region, t.prefix, t.access_key, t.secret_key) = Default::default();
            let _ = crate::secrets::set(&t.secret_name(), None);
        }
        "s3" => {
            let ep = e.endpoint.trim().trim_end_matches('/');
            if !(ep.starts_with("https://") || ep.starts_with("http://")) || ep.contains('@') || ep.len() < 10 {
                return Err("Give the S3 service's address, like https://s3.eu-central-1.amazonaws.com.".into());
            }
            let bucket = e.bucket.trim();
            if bucket.is_empty() || !bucket.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-".contains(c)) {
                return Err("A bucket name is lowercase letters, digits, . and - only.".into());
            }
            if e.access_key.trim().is_empty() {
                return Err("Give the access key.".into());
            }
            t.endpoint = ep.to_string();
            t.bucket = bucket.to_string();
            t.region = if e.region.trim().is_empty() { "us-east-1".into() } else { e.region.trim().to_string() };
            t.prefix = e.prefix.trim().trim_matches('/').to_string();
            t.access_key = e.access_key.trim().to_string();
            t.folder.clear();
            let secret = e.secret_key.trim();
            if !secret.is_empty() {
                if crate::secrets::enabled() {
                    crate::secrets::set(&t.secret_name(), Some(secret))?;
                    t.secret_key.clear();
                } else {
                    t.secret_key = secret.to_string();
                }
            }
            if t.secret().is_none() {
                return Err("Give the secret key.".into());
            }
        }
        _ => return Err("A destination is a folder or an S3 bucket.".into()),
    }
    let id = t.id.clone();
    match at {
        Some(i) => store.api.targets[i] = t,
        None => store.api.targets.push(t),
    }
    Ok(id)
}

/// Removes a destination no account copies to.
pub fn remove_target(store: &mut Store, id: &str) -> Result<(), String> {
    let users: Vec<&str> = store.api.clients.iter().filter(|c| c.copy_to == id).map(|c| c.name.as_str()).collect();
    if !users.is_empty() {
        return Err(format!("{} cop{} reports there: change that first.", users.join(", "), if users.len() == 1 { "ies" } else { "y" }));
    }
    let t = store.api.targets.iter().find(|t| t.id == id).cloned().ok_or_else(|| format!("There is no destination “{id}”."))?;
    let _ = crate::secrets::set(&t.secret_name(), None);
    store.api.targets.retain(|t| t.id != id);
    Ok(())
}

/// Whether a callback may go to `url` for this account: http(s), to one of its hosts.
pub fn callback_ok(c: &ApiClient, url: &str) -> Result<(), String> {
    if c.callback_hosts.is_empty() {
        return Err("Callbacks aren't allowed for this API account (Admin → API: the hosts they may go to).".into());
    }
    if c.sign_key.is_empty() {
        return Err("This API account's key is older than callbacks: ask for a new key (Admin → API) to sign them.".into());
    }
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).ok_or("callback_url must start with https:// or http://.")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_lowercase();
    if host.is_empty() || host.contains('@') || url.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return Err("callback_url isn't an address.".into());
    }
    let bare = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|d| d.is_ascii_digit())).map(|(h, _)| h).unwrap_or(&host);
    if c.callback_hosts.iter().any(|h| *h == host || *h == bare) {
        Ok(())
    } else {
        Err(format!("Callbacks may go to {} only (Admin → API).", c.callback_hosts.join(", ")))
    }
}

/// Sets what a published report accepts over the API; `defs` are its parameters (client and
/// period aside), which the settings must name.
pub fn set_report(store: &mut Store, id: &str, r: ApiReport, defs: &[crate::model::ParamDef]) -> Result<(), String> {
    if !store.reports.iter().any(|p| p.id == id) {
        return Err(format!("There is no published report “{id}”."));
    }
    let mut r = r;
    for (name, p) in &mut r.params {
        let def = defs.iter().find(|d| &d.name == name).ok_or_else(|| format!("The report has no parameter “{name}”."))?;
        if def.multi {
            if let Json::String(s) = &p.default {
                // a list default typed as text: its values by comma
                let v: Vec<&str> = s.split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
                p.default = if v.is_empty() { Json::Null } else { json!(v) };
            }
        } else if matches!(&p.default, Json::Array(_)) {
            return Err(format!("“{name}” takes one value."));
        }
        if p.default == json!("") {
            p.default = Json::Null;
        }
    }
    r.params.retain(|_, p| p.accept || !p.default.is_null());
    if r == ApiReport::default() {
        store.api.reports.remove(id);
    } else {
        store.api.reports.insert(id.to_string(), r);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(name: &str) -> (Store, String) {
        let mut s = crate::auth::seed();
        let key = save(&mut s, ApiClientEdit { name: name.into(), active: true, access: Access { all: false, coverage: vec!["S0001".into()], ..Default::default() }, ..Default::default() }, "admin").unwrap().unwrap();
        (s, key)
    }

    /// A new account gets a key once; the key signs it in as a report user with its access; only a
    /// hash is kept; a new key replaces the old; a switched-off account or a wrong key is refused.
    #[test]
    fn keys_sign_api_accounts_in() {
        let (mut s, key) = store_with("Client portal");
        assert!(key.starts_with("qk.client-portal.") && key.len() > 50, "{key}");
        let stored = serde_json::to_string(&s.api).unwrap();
        assert!(!stored.contains(key.rsplit('.').next().unwrap()), "the secret isn't kept");
        let (c, sess) = sign_in(&s, &key).unwrap();
        assert_eq!(c.id, "client-portal");
        assert_eq!(sess.login, "api:client-portal");
        assert!(sess.has("user") && !sess.has("design") && !sess.has("admin"));
        assert_eq!(sess.access.coverage, vec!["S0001".to_string()]);
        assert!(c.public().get("key_hash").is_none() && c.public()["key"]["hint"].as_str().is_some_and(|h| key.ends_with(h)));
        // wrong keys
        for bad in ["", "qk.client-portal.nope", &key.replace("qk.", "xx."), &format!("{key}x"), "qk.other.abc"] {
            assert_eq!(sign_in(&s, bad).unwrap_err(), "Invalid API key.", "{bad}");
        }
        // a new key: the old one stops working
        let key2 = new_key(&mut s, "client-portal").unwrap();
        assert!(sign_in(&s, &key).is_err() && sign_in(&s, &key2).is_ok());
        // switched off
        let mut e = ApiClientEdit { was: "client-portal".into(), name: "Client portal".into(), active: false, ..Default::default() };
        assert_eq!(save(&mut s, e.clone(), "admin").unwrap(), None, "a change keeps the key");
        assert!(sign_in(&s, &key2).unwrap_err().contains("switched off"));
        // renamed: the key carries the id, so a new key comes with it
        e.id = "portal".into();
        e.active = true;
        let key3 = save(&mut s, e, "admin").unwrap().unwrap();
        assert!(key3.starts_with("qk.portal.") && sign_in(&s, &key3).is_ok() && sign_in(&s, &key2).is_err());
        // clashes and bad ids
        assert!(save(&mut s, ApiClientEdit { name: "Portal".into(), ..Default::default() }, "admin").is_err());
        assert!(save(&mut s, ApiClientEdit { id: "Bad Id".into(), name: "X".into(), ..Default::default() }, "admin").is_err());
        assert!(save(&mut s, ApiClientEdit { name: "X".into(), reports: vec!["nope".into()], ..Default::default() }, "admin").is_err());
        remove(&mut s, "portal").unwrap();
        assert!(sign_in(&s, &key3).is_err());
    }

    /// Callbacks: only to the hosts Admin allows, over http(s), and signed with a key the caller can
    /// work out from its API key.
    #[test]
    fn callbacks_go_only_where_allowed() {
        let (mut s, key) = store_with("CRM");
        assert_eq!(s.api.clients[0].sign_key, sign_key_of(&key));
        assert!(callback_ok(&s.api.clients[0], "https://crm.bank.local/hook").unwrap_err().contains("aren't allowed"));
        let e = ApiClientEdit { was: "crm".into(), name: "CRM".into(), active: true, callback_hosts: vec!["CRM.bank.local".into(), " ".into(), "hooks.bank.local:8443".into()], ..Default::default() };
        save(&mut s, e, "admin").unwrap();
        let c = &s.api.clients[0];
        assert_eq!(c.callback_hosts, vec!["crm.bank.local".to_string(), "hooks.bank.local:8443".to_string()]);
        assert!(callback_ok(c, "https://crm.bank.local/api/quire?x=1").is_ok());
        assert!(callback_ok(c, "http://crm.bank.local:8080/hook").is_ok(), "a host allowed without a port: any port");
        assert!(callback_ok(c, "https://hooks.bank.local:8443/x").is_ok());
        assert!(callback_ok(c, "https://hooks.bank.local/x").is_err(), "a host allowed with a port: that port only");
        for bad in ["https://evil.example/hook", "https://crm.bank.local@evil.example/", "ftp://crm.bank.local/x", "crm.bank.local/x", "https://crm.bank.local.evil.example/"] {
            assert!(callback_ok(c, bad).is_err(), "{bad}");
        }
        assert!(save(&mut s, ApiClientEdit { was: "crm".into(), name: "CRM".into(), callback_hosts: vec!["bad host!".into()], ..Default::default() }, "admin").is_err());
        assert!(save(&mut s, ApiClientEdit { was: "crm".into(), name: "CRM".into(), copy_to: "nowhere".into(), ..Default::default() }, "admin").is_err());
    }

    /// Destinations: a folder by its full path, or an S3 bucket with its keys; the secret is never
    /// shown; one in use can't be removed.
    #[test]
    fn destinations_are_checked() {
        let (mut s, _) = store_with("CRM");
        let folder = |f: &str| ApiTargetEdit { name: "Reports share".into(), kind: "folder".into(), folder: f.into(), ..Default::default() };
        assert!(save_target(&mut s, folder("reports"), "admin").is_err(), "a relative folder");
        let id = save_target(&mut s, folder("/mnt/reports"), "admin").unwrap();
        assert_eq!(id, "reports-share");
        let s3 = ApiTargetEdit { name: "Archive".into(), kind: "s3".into(), endpoint: "https://s3.example.com/".into(), bucket: "quire-reports".into(), prefix: "/api/".into(), access_key: "AK".into(), secret_key: "SECRET".into(), ..Default::default() };
        let s3id = save_target(&mut s, s3.clone(), "admin").unwrap();
        let t = s.api.targets.iter().find(|t| t.id == s3id).unwrap();
        assert_eq!((t.endpoint.as_str(), t.region.as_str(), t.prefix.as_str()), ("https://s3.example.com", "us-east-1", "api"));
        assert_eq!(t.secret().as_deref(), Some("SECRET"));
        assert!(t.public().get("secret_key").is_none() && t.public()["has_secret"] == true);
        // a change without the secret keeps it
        let mut again = s3.clone();
        again.was = s3id.clone();
        again.secret_key.clear();
        save_target(&mut s, again, "admin").unwrap();
        assert_eq!(s.api.targets.iter().find(|t| t.id == s3id).unwrap().secret().as_deref(), Some("SECRET"));
        for bad in [ApiTargetEdit { bucket: "Bad_Bucket".into(), ..s3.clone() }, ApiTargetEdit { endpoint: "s3.example.com".into(), ..s3.clone() }, ApiTargetEdit { access_key: "".into(), ..s3.clone() }, ApiTargetEdit { secret_key: "".into(), ..s3.clone() }] {
            assert!(save_target(&mut s, bad, "admin").is_err());
        }
        // in use: not removed
        save(&mut s, ApiClientEdit { was: "crm".into(), name: "CRM".into(), active: true, copy_to: id.clone(), ..Default::default() }, "admin").unwrap();
        assert!(remove_target(&mut s, &id).unwrap_err().contains("CRM"));
        remove_target(&mut s, &s3id).unwrap();
        assert_eq!(s.api.targets.len(), 1);
    }

    /// A report's API settings name only its parameters; a list's default typed as text becomes its
    /// values; settings back to nothing leave no entry.
    #[test]
    fn report_settings_are_checked() {
        let mut s = crate::auth::seed();
        let defs = crate::uhnwi_sample::template().param_defs;
        let mut r = ApiReport::default();
        r.params.insert("product".into(), ApiParam { accept: true, default: json!("BROK, BANK") });
        set_report(&mut s, "uhnwi", r.clone(), &defs).unwrap();
        assert_eq!(s.api.reports["uhnwi"].params["product"].default, json!(["BROK", "BANK"]));
        r.params.insert("colour".into(), ApiParam { accept: true, default: Json::Null });
        assert!(set_report(&mut s, "uhnwi", r, &defs).is_err());
        assert!(set_report(&mut s, "nope", ApiReport::default(), &defs).is_err());
        set_report(&mut s, "uhnwi", ApiReport::default(), &defs).unwrap();
        assert!(s.api.reports.is_empty());
    }
}
