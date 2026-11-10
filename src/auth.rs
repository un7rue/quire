//! Sign-in, roles and client access.
//!
//! Accounts live in `users.json` in the app's data folder (never in the bank's
//! database): each has a login, a salted password hash, role marks and the
//! clients it may report on. Four roles open the four areas of the app:
//!
//! - **Admin** — users, passwords, roles and access (and every other role);
//! - **Data** — connections and the SQL workspace;
//! - **Design** — templates and themes;
//! - **User** — reports for clients.
//!
//! Access has two parts. **Clients**: every client, or the union of clients
//! covered by given bankers (`clients.sales_id`) and clients named one by one.
//! **Products**: every source system, or only some (`accounts.src_cd`) — the
//! person then sees only clients with such an account, and their reports show
//! only those accounts (queries read `:access_src`). Read from the bank
//! database; it narrows any report parameter that picks a client.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

use crate::model::Connection;

/// The password the demo accounts start with.
pub const DEMO_PASSWORD: &str = "quire";
const ROUNDS: u32 = 120_000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Roles {
    pub admin: bool,
    pub data: bool,
    pub design: bool,
    pub user: bool,
}

impl Roles {
    /// Admin carries every other role.
    pub fn effective(&self) -> Roles {
        if self.admin {
            Roles { admin: true, data: true, design: true, user: true }
        } else {
            self.clone()
        }
    }
    pub fn has(&self, role: &str) -> bool {
        let r = self.effective();
        match role {
            "admin" => r.admin,
            "data" => r.data,
            "design" => r.design,
            "user" => r.user,
            _ => false,
        }
    }
    pub fn any(&self) -> bool {
        let r = self.effective();
        r.admin || r.data || r.design || r.user
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Access {
    /// Every client (Admin always has it).
    pub all: bool,
    /// Bankers whose clients are included (`sales.id`, matched to `clients.sales_id`).
    pub coverage: Vec<String>,
    /// Source systems: clients with an account there are included (`accounts.src_cd`).
    pub sources: Vec<String>,
    /// Clients included one by one (`clients.id`).
    pub clients: Vec<String>,
}

impl Default for Access {
    fn default() -> Self {
        Access { all: true, coverage: vec![], sources: vec![], clients: vec![] }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Account {
    pub login: String,
    pub name: String,
    pub email: String,
    pub salt: String,
    pub hash: String,
    pub roles: Roles,
    pub access: Access,
    /// A disabled account can't sign in.
    pub active: bool,
    /// Asked to choose a new password at the next sign-in.
    pub must_change: bool,
    /// What AI may send for this person: "structure", "aggregates" or "full"; "" = their groups', else the bank's.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ai_level: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Store {
    pub users: Vec<Account>,
    /// Connections of the Data area, shared by everyone with the Data role.
    pub connections: Vec<Connection>,
    /// The bank database client access is read from ("" = the demo database).
    pub access_db: String,
    /// Shared data sources of the Data area (each on one of `connections`).
    pub datasets: Vec<crate::model::Dataset>,
    /// Which saved templates use each shared source (id → templates), for the
    /// warning before one is changed or deleted. Kept up to date as templates
    /// open and save on this computer.
    pub usage: std::collections::BTreeMap<String, Vec<UseRef>>,
    pub groups: Vec<Group>,
    /// Templates published to the Reports area, and who may use each.
    pub reports: Vec<Published>,
    /// Data sources added or changed in the Data area for a published report
    /// (report id → sources). They apply wherever the report is used, reach the
    /// template when its designer opens it, and are folded in when it is published.
    pub report_sources: std::collections::BTreeMap<String, Vec<crate::model::Dataset>>,
    /// The bank's AI rules: what may be sent and to which providers.
    pub ai: crate::ai::AiPolicy,
    /// The web server runs Python data sources and scripts (off until an administrator says so).
    pub python_on_server: bool,
    /// The report API: the systems that ask for reports, and what each report accepts from them.
    pub api: crate::api_accounts::ApiSettings,
}

/// Puts the Data area's changes for a report into its pages (by source id).
pub fn apply_report_sources(st: &Store, id: &str, doc: &mut crate::model::Doc) -> usize {
    let Some(list) = st.report_sources.get(id) else { return 0 };
    for d in list {
        match doc.datasets.iter_mut().find(|x| x.id == d.id) {
            Some(x) => *x = d.clone(),
            None => doc.datasets.push(d.clone()),
        }
        // its connection comes along (from the Data area's shared ones)
        if let crate::model::Source::Db { connection, .. } = &d.source {
            if !doc.connections.iter().any(|c| &c.id == connection) {
                if let Some(c) = st.connections.iter().find(|c| &c.id == connection) {
                    doc.connections.push(c.clone());
                }
            }
        }
    }
    list.len()
}

/// People who share work: a report can be given to a group instead of person by person.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Logins.
    pub members: Vec<String>,
    /// What AI may send for its members ("" = not set: the bank's). A person in several groups gets the widest.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ai_level: String,
}

/// Who may use a published report (people with the User role).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Audience {
    /// Everyone with the User role.
    pub everyone: bool,
    /// Group ids.
    pub groups: Vec<String>,
    /// Logins.
    pub users: Vec<String>,
}

/// A template published to the Reports area: report users create reports from it.
/// Its pages are a snapshot taken when it is published (`reports/<id>.rtpl`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Published {
    pub id: String,
    pub title: String,
    pub description: String,
    pub audience: Audience,
    /// Shown in the Reports area (false: a draft — kept, but nobody sees it).
    pub published: bool,
    /// Who published it last, and when.
    pub by: String,
    pub at: String,
    pub version: u32,
    /// A built-in template used while no snapshot was published ("uhnwi").
    pub builtin: String,
}

impl Store {
    /// Whether this person may create reports from `p`: designers and admins
    /// always (they manage reports); report users when it's published to them.
    pub fn may_use(&self, p: &Published, s: &Session) -> bool {
        if s.has("admin") || s.has("design") {
            return true;
        }
        if !s.has("user") || !p.published {
            return false;
        }
        let a = &p.audience;
        a.everyone
            || a.users.iter().any(|u| u.eq_ignore_ascii_case(&s.login))
            || self.groups.iter().any(|g| a.groups.contains(&g.id) && g.members.iter().any(|m| m.eq_ignore_ascii_case(&s.login)))
    }

    /// The people an audience reaches (report users, by login).
    pub fn reach(&self, a: &Audience) -> Vec<String> {
        let mut out: Vec<String> = self
            .users
            .iter()
            .filter(|u| u.active && u.roles.effective().user)
            .filter(|u| {
                a.everyone
                    || a.users.iter().any(|x| x.eq_ignore_ascii_case(&u.login))
                    || self.groups.iter().any(|g| a.groups.contains(&g.id) && g.members.iter().any(|m| m.eq_ignore_ascii_case(&u.login)))
            })
            .map(|u| u.login.clone())
            .collect();
        out.sort();
        out
    }
}

fn seed_groups() -> Vec<Group> {
    vec![
        Group { id: "bankers".into(), name: "Private bankers".into(), description: "Relationship managers who send reports to their clients".into(), members: vec!["user".into()], ai_level: String::new() },
        Group { id: "insurance".into(), name: "Insurance desk".into(), description: "Policies and savings plans".into(), members: vec![], ai_level: String::new() },
        Group { id: "analysts".into(), name: "Analysts".into(), description: "Data and reporting team".into(), members: vec!["data".into()], ai_level: String::new() },
    ]
}

fn seed_reports() -> Vec<Published> {
    vec![Published {
        id: "uhnwi".into(),
        title: "Modern UHNWI report".into(),
        description: "Private-banking report in Russian: portfolio, results, benchmarks, accounts, holdings, forecast and trades.".into(),
        audience: Audience { everyone: false, groups: vec!["bankers".into()], users: vec![] },
        published: true,
        by: "Quire".into(),
        at: String::new(),
        version: 1,
        builtin: "uhnwi".into(),
    }]
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct UseRef {
    pub title: String,
    pub path: String,
    /// Widgets in that template reading the source.
    pub widgets: usize,
}

/// A few shared sources on the demo bank to start from.
fn seed_datasets() -> Vec<crate::model::Dataset> {
    use crate::model::{Dataset, Source};
    let ds = |id: &str, name: &str, date: Option<&str>, snapshot: bool, q: &str| Dataset {
        id: id.into(),
        name: name.into(),
        source: Source::Db { connection: "demo".into(), query: q.trim().into() },
        date_field: date.map(String::from),
        snapshot,
        shared: true,
        ..Default::default()
    };
    vec![
        ds("s_clients", "Клиенты и банкиры", None, false, r#"
select c.id as client_id, c.name as client, c.birthday, c.nationality, c.risk_profile,
       s.name as banker, s.sales_email as banker_email, s.sales_phone as banker_phone
from clients c left join sales s on s.id = c.sales_id"#),
        ds("s_accounts", "Счета клиентов", None, false, r#"
select a.acct_cd, a.acct_numb, a.acct_name, a.acct_type, a.src_cd, a.open_dt, a.close_dt, a.manager, c.name as client
from accounts a join clients c on c.id = a.client_id
where c.name = :client"#),
        ds("s_positions", "Позиции на дату", Some("report_dt"), true, r#"
select b.report_dt, a.acct_numb, a.src_cd, i.instr_name, i.instr_type_ru, b.qty, b.price, b.price_cur, b.amt_rub
from balances b join accounts a using (acct_cd) join clients c on c.id = a.client_id join instruments i using (instr_cd)
where c.name = :client and b.report_dt between :data_start and :data_end"#),
        ds("s_operations", "Операции", Some("op_dt"), false, r#"
select o.op_dt, a.acct_numb, a.src_cd, o.op_type, i.instr_name, o.qty, o.amount_rub, o.fee
from operations o join accounts a using (acct_cd) join clients c on c.id = o.client_id left join instruments i using (instr_cd)
where c.name = :client and o.op_dt between :data_start and :data_end"#),
    ]
}

/// A store from before shared sources gets the starter ones.
pub fn upgrade(s: &mut Store) -> bool {
    let mut changed = false;
    if s.datasets.is_empty() && s.usage.is_empty() && s.connections.iter().any(|c| c.id == "demo") {
        s.datasets = seed_datasets();
        changed = true;
    }
    if s.groups.is_empty() && s.reports.is_empty() {
        s.groups = seed_groups();
        s.reports = seed_reports();
        changed = true;
    }
    changed
}

/// Who is signed in.
#[derive(Serialize, Clone, Debug)]
pub struct Session {
    pub login: String,
    pub name: String,
    pub roles: Roles,
    pub access: Access,
}

impl Session {
    pub fn has(&self, role: &str) -> bool {
        self.roles.has(role)
    }
    /// The source systems this person's reports show ("" = every one).
    pub fn products(&self) -> String {
        if self.roles.admin { String::new() } else { self.access.sources.join(",") }
    }
}

// ---------------------------------------------------------------- passwords

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn new_salt() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        // never without a salt: fall back to the clock
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        b.copy_from_slice(&t.to_le_bytes());
    }
    hex(&b)
}

/// The salted, stretched hash of a password.
pub fn hash(salt: &str, password: &str) -> String {
    let mut h: [u8; 32] = Sha256::digest(format!("{salt}\u{0}{password}").as_bytes()).into();
    for _ in 0..ROUNDS {
        let mut d = Sha256::new();
        d.update(h);
        d.update(salt.as_bytes());
        h = d.finalize().into();
    }
    hex(&h)
}

impl Account {
    pub fn set_password(&mut self, password: &str) {
        self.salt = new_salt();
        self.hash = hash(&self.salt, password);
    }
    pub fn check(&self, password: &str) -> bool {
        let h = hash(&self.salt, password);
        // compare in constant time
        h.len() == self.hash.len() && h.bytes().zip(self.hash.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0
    }
    pub fn session(&self) -> Session {
        let roles = self.roles.effective();
        let access = if roles.admin { Access::default() } else { self.access.clone() };
        Session { login: self.login.clone(), name: if self.name.is_empty() { self.login.clone() } else { self.name.clone() }, roles, access }
    }
    /// What the admin screen shows (no hash, no salt).
    pub fn public(&self) -> Json {
        json!({
            "login": self.login, "name": self.name, "email": self.email,
            "roles": self.roles, "access": self.access, "active": self.active, "must_change": self.must_change, "ai_level": self.ai_level,
            "demo_password": self.check(DEMO_PASSWORD),
        })
    }
}

// ---------------------------------------------------------------- the store

/// The demo accounts: one per role. The user is a banker who sees the
/// clients they cover.
pub fn seed() -> Store {
    let mk = |login: &str, name: &str, email: &str, roles: Roles, access: Access| {
        let mut a = Account { login: login.into(), name: name.into(), email: email.into(), roles, access, active: true, ..Default::default() };
        a.set_password(DEMO_PASSWORD);
        a
    };
    let all = Access::default();
    Store {
        users: vec![
            mk("admin", "Alex Morgan", "", Roles { admin: true, ..Default::default() }, all.clone()),
            mk("data", "Dana Fields", "", Roles { data: true, ..Default::default() }, all.clone()),
            mk("design", "Iris Lane", "", Roles { design: true, ..Default::default() }, all.clone()),
            mk("user", "Warren Buffington", "", Roles { user: true, ..Default::default() }, Access { all: false, coverage: vec!["S0001".into()], ..Default::default() }),
        ],
        connections: crate::dbconn::demo_path()
            .map(|p| vec![Connection { id: "demo".into(), name: "Demo bank".into(), kind: "duckdb".into(), path: p, url: String::new() }])
            .unwrap_or_default(),
        access_db: String::new(),
        datasets: if crate::dbconn::demo_path().is_some() { seed_datasets() } else { vec![] },
        usage: Default::default(),
        groups: seed_groups(),
        reports: seed_reports(),
        report_sources: Default::default(),
        ai: Default::default(),
        python_on_server: false,
        api: Default::default(),
    }
}

pub fn load(path: &Path) -> Store {
    match std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Store>(&t).ok()) {
        Some(mut s) if !s.users.is_empty() => {
            // the server keeps connection passwords in its secrets file: back into the addresses
            if crate::secrets::enabled() {
                for c in &mut s.connections {
                    if split_password(&c.url).1.is_none() {
                        if let Some(pw) = crate::secrets::get(&format!("conn.{}", c.id)) {
                            c.url = with_password(&c.url, &pw);
                        }
                    }
                }
            }
            if upgrade(&mut s) {
                let _ = save(path, &s);
            }
            s
        }
        _ => {
            let s = seed();
            let _ = save(path, &s);
            s
        }
    }
}

/// Writes the store whole (never half a file, one writer at a time). On the server, connection
/// passwords go to the secrets file and the addresses are written without them.
pub fn save(path: &Path, s: &Store) -> Result<(), String> {
    static SAVING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = SAVING.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = s.clone();
    if crate::secrets::enabled() {
        for c in &mut out.connections {
            let (bare, pw) = split_password(&c.url);
            crate::secrets::set(&format!("conn.{}", c.id), pw.as_deref())?;
            c.url = bare;
        }
    }
    crate::secrets::write_private(path, serde_json::to_string_pretty(&out).unwrap_or_default().as_bytes())
}

/// What a connection's password shows as where secrets are kept on the server.
pub const HIDDEN: &str = "••••••";

/// A connection address without its password (`postgres://ann@host/db`), and the password.
pub fn split_password(url: &str) -> (String, Option<String>) {
    let Some(s) = url.find("://") else { return (url.to_string(), None) };
    let rest = &url[s + 3..];
    let Some(at) = rest.rfind('@') else { return (url.to_string(), None) };
    let Some(colon) = rest[..at].find(':') else { return (url.to_string(), None) };
    let pw = &rest[colon + 1..at];
    (format!("{}{}{}", &url[..s + 3], &rest[..colon], &rest[at..]), (!pw.is_empty()).then(|| pw.to_string()))
}

/// The address with `pw` as its password.
pub fn with_password(url: &str, pw: &str) -> String {
    let (bare, _) = split_password(url);
    let Some(s) = bare.find("://") else { return bare };
    let rest = &bare[s + 3..];
    match rest.rfind('@') {
        Some(at) => format!("{}{}:{pw}{}", &bare[..s + 3], &rest[..at], &rest[at..]),
        None => bare,
    }
}

/// Failed sign-ins per login: after `TRIES` in a row the login rests for `REST`.
const TRIES: u32 = 5;
const REST: std::time::Duration = std::time::Duration::from_secs(15 * 60);

fn failures() -> std::sync::MutexGuard<'static, std::collections::HashMap<String, (u32, std::time::Instant)>> {
    static F: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (u32, std::time::Instant)>>> = std::sync::OnceLock::new();
    F.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether a login may try to sign in now (not resting after too many wrong passwords).
pub fn sign_in_allowed(login: &str) -> Result<(), String> {
    let mut f = failures();
    let key = login.trim().to_lowercase();
    if let Some((n, at)) = f.get(&key).copied() {
        if at.elapsed() >= REST {
            f.remove(&key);
        } else if n >= TRIES {
            let min = (REST - at.elapsed()).as_secs().div_ceil(60);
            return Err(format!("Too many wrong passwords. Try again in {min} min."));
        }
    }
    Ok(())
}

pub fn sign_in_failed(login: &str) {
    let mut f = failures();
    let e = f.entry(login.trim().to_lowercase()).or_insert((0, std::time::Instant::now()));
    e.0 += 1;
    e.1 = std::time::Instant::now();
}

pub fn sign_in_ok(login: &str) {
    failures().remove(&login.trim().to_lowercase());
}

pub fn sign_in(s: &Store, login: &str, password: &str) -> Result<Session, String> {
    let bad = || "Wrong login or password.".to_string();
    let a = s.users.iter().find(|a| a.login.eq_ignore_ascii_case(login.trim())).ok_or_else(bad)?;
    if !a.check(password) {
        return Err(bad());
    }
    if !a.active {
        return Err("This account is disabled. Ask an administrator.".into());
    }
    if !a.roles.any() {
        return Err("This account has no role yet. Ask an administrator.".into());
    }
    Ok(a.session())
}

/// An admin's change to an account (a new one when `login` is new).
#[derive(Deserialize, Debug, Clone)]
pub struct AccountEdit {
    /// The account's current login ("" for a new account).
    #[serde(default)]
    pub was: String,
    pub login: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub roles: Roles,
    #[serde(default)]
    pub access: Access,
    #[serde(default = "yes")]
    pub active: bool,
    /// A new password ("" keeps the current one).
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub must_change: bool,
    /// What AI may send for this person ("" = their groups', else the bank's).
    #[serde(default)]
    pub ai_level: String,
}

fn yes() -> bool {
    true
}

fn admins(s: &Store) -> usize {
    s.users.iter().filter(|a| a.active && a.roles.admin).count()
}

/// Applies an admin's edit; `me` is the admin making it.
pub fn upsert(store: &mut Store, e: AccountEdit, me: &str) -> Result<(), String> {
    // all or nothing: work on a copy
    let mut s = store.clone();
    let s = &mut s;
    let login = e.login.trim().to_string();
    if login.is_empty() || !login.chars().all(|c| c.is_alphanumeric() || "._-@".contains(c)) {
        return Err("A login is letters, digits and . _ - @ only.".into());
    }
    let clash = s.users.iter().any(|a| a.login.eq_ignore_ascii_case(&login) && !a.login.eq_ignore_ascii_case(&e.was));
    if clash {
        return Err(format!("There is already an account “{login}”."));
    }
    let at = if e.was.is_empty() { None } else { s.users.iter().position(|a| a.login.eq_ignore_ascii_case(&e.was)) };
    if at.is_none() && e.password.len() < 4 {
        return Err("Give the new account a password (4 characters or more).".into());
    }
    if !e.password.is_empty() && e.password.len() < 4 {
        return Err("A password needs 4 characters or more.".into());
    }
    let mut a = at.map(|i| s.users[i].clone()).unwrap_or_default();
    if a.login.eq_ignore_ascii_case(me) && (!e.roles.admin || !e.active) {
        return Err("You can't take the Admin role from yourself or disable your own account.".into());
    }
    a.login = login;
    a.name = e.name.trim().to_string();
    a.email = e.email.trim().to_string();
    a.roles = e.roles;
    a.access = e.access;
    a.active = e.active;
    a.must_change = e.must_change;
    a.ai_level = ai_level_ok(&e.ai_level)?;
    if !e.password.is_empty() {
        a.set_password(&e.password);
    }
    match at {
        Some(i) => s.users[i] = a,
        None => s.users.push(a),
    }
    if admins(s) == 0 {
        return Err("Keep at least one active administrator.".into());
    }
    *store = s.clone();
    Ok(())
}

/// A level as stored: one of the three, or "" (not set).
pub fn ai_level_ok(v: &str) -> Result<String, String> {
    match v.trim() {
        "" | "structure" | "aggregates" | "full" => Ok(v.trim().to_string()),
        other => Err(format!("“{other}” is not a level for AI (structure, aggregates or full).")),
    }
}

fn level_rank(v: &str) -> u8 {
    match v {
        "full" => 2,
        "aggregates" => 1,
        _ => 0,
    }
}

/// What AI may send for a person, and where that comes from: their own setting ("user"),
/// the widest of their groups' ("group", with its name), else the bank's ("bank").
pub fn ai_level_for(store: &Store, login: &str) -> (String, &'static str, String) {
    if let Some(a) = store.users.iter().find(|a| a.login.eq_ignore_ascii_case(login)).filter(|a| !a.ai_level.is_empty()) {
        return (a.ai_level.clone(), "user", String::new());
    }
    let best = store.groups.iter()
        .filter(|g| !g.ai_level.is_empty() && g.members.iter().any(|m| m.eq_ignore_ascii_case(login)))
        .max_by_key(|g| level_rank(&g.ai_level));
    match best {
        Some(g) => (g.ai_level.clone(), "group", g.name.clone()),
        None => (store.ai.level.clone(), "bank", String::new()),
    }
}

pub fn remove(store: &mut Store, login: &str, me: &str) -> Result<(), String> {
    let mut s = store.clone();
    let s = &mut s;
    if login.eq_ignore_ascii_case(me) {
        return Err("You can't delete your own account.".into());
    }
    let before = s.users.len();
    s.users.retain(|a| !a.login.eq_ignore_ascii_case(login));
    if s.users.len() == before {
        return Err(format!("No account “{login}”."));
    }
    if admins(s) == 0 {
        return Err("Keep at least one active administrator.".into());
    }
    *store = s.clone();
    Ok(())
}

pub fn change_password(s: &mut Store, login: &str, old: &str, new: &str) -> Result<(), String> {
    let a = s.users.iter_mut().find(|a| a.login.eq_ignore_ascii_case(login)).ok_or("No such account.")?;
    if !a.check(old) {
        return Err("The current password is wrong.".into());
    }
    if new.len() < 4 {
        return Err("A password needs 4 characters or more.".into());
    }
    if new == old {
        return Err("Choose a password different from the current one.".into());
    }
    a.set_password(new);
    a.must_change = false;
    Ok(())
}

// ---------------------------------------------------------------- client access

pub(crate) fn db_path(s: &Store) -> Option<String> {
    if !s.access_db.trim().is_empty() {
        return Some(crate::data::resolve_path(s.access_db.trim(), None).display().to_string());
    }
    crate::dbconn::demo_path()
}

pub(crate) fn quote_list(v: &[String]) -> String {
    v.iter().map(|x| format!("'{}'", x.replace('\'', "''"))).collect::<Vec<_>>().join(", ")
}

pub(crate) fn query(path: &str, sql: &str, n: usize) -> Result<Vec<Vec<String>>, String> {
    let conn = duckdb::Connection::open_with_flags(path, duckdb::Config::default().access_mode(duckdb::AccessMode::ReadOnly).map_err(|e| e.to_string())?)
        .map_err(|e| format!("The bank database can't be opened: {e}"))?;
    let mut st = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| {
            Ok((0..n).map(|i| r.get::<_, Option<String>>(i).ok().flatten().unwrap_or_default()).collect::<Vec<String>>())
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// The clients an access rule allows, as (id, name); `None` = every client.
/// Who (every client, or bankers' coverage plus clients picked one by one)
/// meets what (every product, or the chosen source systems: only clients
/// with an account there).
pub fn allowed(s: &Store, a: &Access) -> Result<Option<Vec<(String, String)>>, String> {
    if a.all && a.sources.is_empty() {
        return Ok(None);
    }
    let mut ands = Vec::new();
    if !a.all {
        let mut ors = Vec::new();
        if !a.coverage.is_empty() {
            ors.push(format!("sales_id in ({})", quote_list(&a.coverage)));
        }
        if !a.clients.is_empty() {
            ors.push(format!("id in ({})", quote_list(&a.clients)));
        }
        if ors.is_empty() {
            return Ok(Some(vec![]));
        }
        ands.push(format!("({})", ors.join(" or ")));
    }
    if !a.sources.is_empty() {
        ands.push(format!("id in (select client_id from accounts where src_cd in ({}))", quote_list(&a.sources)));
    }
    let path = db_path(s).ok_or("No bank database to read client access from.")?;
    let rows = query(&path, &format!("select id, name from clients where {} order by name", ands.join(" and ")), 2)?;
    Ok(Some(rows.into_iter().map(|r| (r[0].clone(), r[1].clone())).collect()))
}

/// What the admin picks access from: bankers, source systems and clients.
pub fn options(s: &Store) -> Result<Json, String> {
    let path = db_path(s).ok_or("No bank database to read client access from.")?;
    let bankers = query(&path, "select s.id, s.name, count(c.id)::varchar from sales s left join clients c on c.sales_id = s.id group by 1, 2 order by 2", 3)?;
    // the products accounts use, named from the bank's product table when it has one (else by their code)
    let sources = query(&path, "select a.src_cd, count(distinct a.client_id)::varchar, coalesce(p.name, ''), coalesce(p.description, ''), coalesce(p.name_en, ''), coalesce(p.description_en, '')
        from accounts a left join products p on p.src_cd = a.src_cd group by all order by min(coalesce(p.sort_order, 999)), 1", 6)
        .or_else(|_| query(&path, "select src_cd, count(distinct client_id)::varchar, '', '', '', '' from accounts group by 1 order by 1", 6))?;
    let clients = query(&path, "select c.id, c.name, coalesce(s.name, '') from clients c left join sales s on s.id = c.sales_id order by c.name", 3)?;
    Ok(json!({
        "db": path,
        "bankers": bankers.iter().map(|r| json!({"id": r[0], "name": r[1], "clients": r[2].parse::<i64>().unwrap_or(0)})).collect::<Vec<_>>(),
        "sources": sources.iter().map(|r| json!({"id": r[0], "clients": r[1].parse::<i64>().unwrap_or(0), "name": r[2], "description": r[3], "name_en": r[4], "description_en": r[5]})).collect::<Vec<_>>(),
        "clients": clients.iter().map(|r| json!({"id": r[0], "name": r[1], "banker": r[2]})).collect::<Vec<_>>(),
    }))
}

/// Whether a report parameter picks a client: named client / client_id /
/// client_name, or reading its choices from such a column.
pub fn is_client_param(d: &crate::model::ParamDef) -> bool {
    let is = |s: &str| matches!(s.to_lowercase().as_str(), "client" | "client_id" | "client_name" | "клиент");
    is(&d.name) || d.source.as_ref().map(|s| is(&s.field)).unwrap_or(false)
}

#[cfg(test)]
mod ai_level_tests {
    use super::*;

    /// A person's own setting first; else the widest of their groups'; else the bank's.
    #[test]
    fn ai_level_by_person_group_or_bank() {
        let mut st = seed();
        st.ai.level = "structure".into();
        assert_eq!(ai_level_for(&st, "user").0, "structure");
        assert_eq!(ai_level_for(&st, "user").1, "bank");
        let g = st.groups.iter_mut().find(|g| g.members.contains(&"user".to_string())).unwrap();
        g.ai_level = "aggregates".into();
        let (lvl, from, name) = ai_level_for(&st, "user");
        assert_eq!((lvl.as_str(), from), ("aggregates", "group"));
        assert!(!name.is_empty());
        st.groups.push(Group { id: "x".into(), name: "X".into(), members: vec!["user".into()], ai_level: "full".into(), ..Default::default() });
        assert_eq!(ai_level_for(&st, "user").0, "full", "the widest group wins");
        st.users.iter_mut().find(|a| a.login == "user").unwrap().ai_level = "structure".into();
        assert_eq!(ai_level_for(&st, "user"), ("structure".to_string(), "user", String::new()), "the person's own setting first");
        assert!(ai_level_ok("everything").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_check_and_roles_follow_admin() {
        let mut s = seed();
        assert!(sign_in(&s, "admin", DEMO_PASSWORD).is_ok());
        assert!(sign_in(&s, "ADMIN", DEMO_PASSWORD).is_ok(), "logins ignore case");
        assert!(sign_in(&s, "admin", "nope").is_err());
        let me = sign_in(&s, "admin", DEMO_PASSWORD).unwrap();
        assert!(me.has("data") && me.has("design") && me.has("user"), "admin carries every role");
        let u = sign_in(&s, "user", DEMO_PASSWORD).unwrap();
        assert!(u.has("user") && !u.has("design") && !u.has("admin"));
        // the hash is salted: the same password hashes differently per account
        assert_ne!(s.users[0].hash, s.users[1].hash);
        assert!(!s.users[0].hash.contains(DEMO_PASSWORD));
        // the last admin stays
        assert!(remove(&mut s, "admin", "design").is_err());
        assert!(remove(&mut s, "admin", "admin").is_err(), "not yourself");
        change_password(&mut s, "user", DEMO_PASSWORD, "s3cret").unwrap();
        assert!(sign_in(&s, "user", "s3cret").is_ok() && sign_in(&s, "user", DEMO_PASSWORD).is_err());
        let e = AccountEdit { was: "user".into(), login: "user".into(), name: "W".into(), email: String::new(), roles: Roles::default(), access: Access::default(), active: true, password: String::new(), must_change: false, ai_level: String::new() };
        upsert(&mut s, e, "admin").unwrap();
        assert!(sign_in(&s, "user", "s3cret").is_err(), "no role, no sign-in");
    }

    #[test]
    fn access_by_coverage_source_and_client() {
        let s = seed();
        if db_path(&s).is_none() {
            return;
        }
        let cov = allowed(&s, &Access { all: false, coverage: vec!["S0001".into()], ..Default::default() }).unwrap().unwrap();
        assert_eq!(cov.len(), 6);
        assert!(cov.iter().any(|c| c.0 == "C1337"));
        let ins = allowed(&s, &Access { all: true, sources: vec!["INS".into()], ..Default::default() }).unwrap().unwrap();
        assert!(!ins.is_empty() && ins.len() < 41, "every client holding a policy");
        let both = allowed(&s, &Access { all: false, coverage: vec!["S0001".into()], sources: vec!["INS".into()], ..Default::default() }).unwrap().unwrap();
        assert!(both.len() < 6 && both.iter().all(|c| ins.contains(c)), "a banker's clients with a policy");
        let one = allowed(&s, &Access { all: false, clients: vec!["C0002".into()], coverage: vec!["S0001".into()], ..Default::default() }).unwrap().unwrap();
        assert_eq!(one.len(), 7, "the parts add up");
        assert!(allowed(&s, &Access::default()).unwrap().is_none());
        assert!(allowed(&s, &Access { all: false, ..Default::default() }).unwrap().unwrap().is_empty(), "nothing chosen: no clients");
    }
}

#[cfg(test)]
mod password_tests {
    use super::*;

    #[test]
    fn connection_passwords_come_off_and_back() {
        let (bare, pw) = split_password("postgres://reporter:S3cret@db.bank.local:5432/dwh");
        assert_eq!(bare, "postgres://reporter@db.bank.local:5432/dwh");
        assert_eq!(pw.as_deref(), Some("S3cret"));
        assert_eq!(with_password(&bare, "S3cret"), "postgres://reporter:S3cret@db.bank.local:5432/dwh");
        assert_eq!(with_password("postgres://reporter:old@h/db", HIDDEN), format!("postgres://reporter:{HIDDEN}@h/db"));
        // no user, no password: left alone
        assert_eq!(split_password("clickhouse://host:8123/db"), ("clickhouse://host:8123/db".to_string(), None));
        assert_eq!(split_password("postgres://ann@host/db"), ("postgres://ann@host/db".to_string(), None));
    }
}
