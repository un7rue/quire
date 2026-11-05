//! The administrator's AI assistant: questions about the activity log ("who exported PDFs for
//! this client last week?"), why an event failed, access set up in words (a proposal to confirm,
//! never applied by the model) and an access review. The model reads people, clients and bankers
//! as placeholders ([P-1], [CL-3], [B-2]) — their names, logins and client codes stay here — and
//! the activity as compact lines, numbered so its answer can point at them (S12, E40).

use crate::ai_report::Masker;
use crate::auth::{self, Store};
use serde_json::{json, Value as Json};

/// Everything the assistant may read, already masked, and how to find things again.
pub struct Ctx {
    pub text: String,
    pub masker: Masker,
    /// S1… → session ids; E1… → (at, user) of events.
    pub sessions: Vec<String>,
    pub events: Vec<(String, String)>,
}

/// People (users and bankers) and clients registered in the masker: names are masked wherever
/// they appear; a client's code is never sent (its name's placeholder stands for both). Logins can
/// be ordinary words ("admin", "user"), so they're replaced only where a person is meant: `Who`.
pub struct Who(std::collections::HashMap<String, String>);

impl Who {
    /// A person's placeholder from their login (else the text as it is, masked).
    pub fn of(&self, login: &str, m: &mut Masker) -> String {
        self.0.get(login).cloned().unwrap_or_else(|| m.mask(login))
    }
}

pub fn masker(store: &Store, opts: &Json) -> (Masker, Who) {
    let mut m = Masker::new(&[]);
    let mut who = std::collections::HashMap::new();
    for u in &store.users {
        let t = m.name(if u.name.trim().chars().count() >= 2 { &u.name } else { &u.login }, "P");
        if !u.email.is_empty() {
            m.alias(&u.email, &t);
        }
        who.insert(u.login.clone(), t);
    }
    for b in opts["bankers"].as_array().into_iter().flatten() {
        m.name(b["name"].as_str().unwrap_or(""), "B");
    }
    for c in opts["clients"].as_array().into_iter().flatten() {
        let t = m.name(c["name"].as_str().unwrap_or(""), "CL");
        m.alias(c["id"].as_str().unwrap_or(""), &t);
    }
    (m, Who(who))
}

fn when(at: &str) -> String {
    // "2026-10-21 16:55:35": no letters, so it isn't taken for a code
    at.replace('T', " ").chars().take(19).collect()
}

/// An AI level as the app names it ("aggregates" and the older "full" are both Figures).
fn level_name(l: &str) -> &'static str {
    if matches!(l, "aggregates" | "full") { "Figures" } else { "Structure only" }
}

/// The bank's AI settings, as the administrator sees them in Admin → AI.
fn ai_settings(store: &Store) -> String {
    let p = &store.ai;
    let groups: Vec<String> = store.groups.iter().filter(|g| !g.ai_level.is_empty()).map(|g| format!("group {}: {}", g.name, level_name(&g.ai_level))).collect();
    let own = store.users.iter().filter(|u| !u.ai_level.is_empty()).count();
    format!("AI SETTINGS (Admin → AI). There are exactly two levels of what AI may send: \"Structure only\" (table and column names, types, SQL — what the SQL and design assistants need) \
        and \"Figures\" (also the figures a report shows, with client, account and instrument names and codes masked — what the report assistant needs).\n\
        AI assistants: {} · the bank's level: {} · providers allowed: {} · set for groups: {} · set for people: {}\n\
        A person's own setting wins; otherwise the widest of their groups'; otherwise the bank's.\n",
        if p.off { "switched off" } else { "on" }, level_name(&p.level),
        if p.providers.is_empty() { "all".to_string() } else { p.providers.join(", ") },
        if groups.is_empty() { "none".to_string() } else { groups.join("; ") }, if own == 0 { "none".to_string() } else { format!("{own} people") })
}

/// The people: what each may do and see, and how AI treats them.
fn people(store: &Store, opts: &Json, m: &mut Masker, w: &Who) -> String {
    let mut out = ai_settings(store);
    out.push_str("\nPEOPLE (placeholder · roles · clients · products · groups · what AI may send for them, and why · state)\n");
    for u in &store.users {
        let t = w.of(&u.login, m);
        let roles: Vec<&str> = [("admin", u.roles.admin), ("data", u.roles.data), ("design", u.roles.design), ("user", u.roles.user)].iter().filter(|x| x.1).map(|x| x.0).collect();
        let a = &u.access;
        let clients = if u.roles.admin || a.all {
            "every client".to_string()
        } else {
            let mut parts = Vec::new();
            for b in &a.coverage {
                let name = opts["bankers"].as_array().into_iter().flatten().find(|x| x["id"] == b.as_str()).and_then(|x| x["name"].as_str()).unwrap_or(b);
                parts.push(format!("clients of banker {}", m.mask(name)));
            }
            for c in &a.clients {
                parts.push(m.mask(c));
            }
            if parts.is_empty() { "no clients".into() } else { parts.join(", ") }
        };
        let products = if a.sources.is_empty() { "every product".to_string() } else { a.sources.join(", ") };
        let groups: Vec<String> = store.groups.iter().filter(|g| g.members.iter().any(|x| x == &u.login)).map(|g| format!("{} ({})", g.name, g.id)).collect();
        let (lvl, from, gname) = auth::ai_level_for(store, &u.login);
        let why = match from { "user" => "set for this person".to_string(), "group" => format!("set for the group {gname}"), _ => "the bank's setting".to_string() };
        out.push_str(&format!("{t} · {} · {clients} · {products} · groups: {} · AI: {} ({why}){} · {}{}\n",
            if roles.is_empty() { "no role".into() } else { roles.join("+") },
            if groups.is_empty() { "none".into() } else { groups.join(", ") }, level_name(&lvl), String::new(),
            if u.active { "active" } else { "disabled" },
            if u.must_change { " · must change password" } else { "" }));
    }
    out
}

/// The activity of the last weeks: sessions and events, newest first, numbered.
pub fn activity(store: &Store, opts: &Json, sessions: &Json, events: &Json, extra: &str) -> Ctx {
    let (mut m, w) = masker(store, opts);
    let mut text = people(store, opts, &mut m, &w);
    let mut sids = Vec::new();
    let mut eids = Vec::new();
    text.push_str("\nSESSIONS (number · start–end · person · document · client · period · active time · what was done · how it ended)\n");
    for s in sessions["sessions"].as_array().into_iter().flatten().take(150) {
        sids.push(s["id"].as_str().unwrap_or("").to_string());
        let person = w.of(s["user"].as_str().unwrap_or(""), &mut m);
        let exports: Vec<String> = s["exports"].as_array().into_iter().flatten().map(|e| e["detail"].as_str().unwrap_or("").split(" · ").next().unwrap_or("").to_string()).collect();
        let line = format!("S{} · {}–{} · {} · {} «{}» · client {} · {} · active {}s of {}s · edits {} · saves {} · exports {} · AI {} ({} → {} tokens){} · {}\n",
            sids.len(), when(s["start"].as_str().unwrap_or("")), when(s["end"].as_str().unwrap_or("")).get(11..).unwrap_or(""),
            person, s["kind"].as_str().unwrap_or(""), m.mask(s["title"].as_str().unwrap_or("")), s["client"].as_str().filter(|c| !c.is_empty()).map(|c| m.mask(c)).unwrap_or_else(|| "none".into()),
            s["period"].as_str().unwrap_or(""), s["active_s"], s["wall_s"], s["edits"], s["saves"], if exports.is_empty() { "none".into() } else { exports.join(", ") },
            s["ai"], s["tokens_in"], s["tokens_out"], if s["failed"].as_u64().unwrap_or(0) > 0 { format!(" · {} failed", s["failed"]) } else { String::new() },
            if s["open"] == true { "still open".to_string() } else { s["reason"].as_str().unwrap_or("").to_string() });
        text.push_str(&line);
    }
    text.push_str("\nEVENTS (number · time · person · area · action · on · detail · result)\n");
    for e in events["rows"].as_array().into_iter().flatten().take(400) {
        eids.push((e["at"].as_str().unwrap_or("").to_string(), e["user"].as_str().unwrap_or("").to_string()));
        let detail: String = e["detail"].as_str().unwrap_or("").chars().take(160).collect();
        let person = e["user"].as_str().filter(|u| !u.is_empty()).map(|u| w.of(u, &mut m)).unwrap_or_else(|| "—".into());
        // a sign-in's or an account's target is a login
        let target = e["target"].as_str().unwrap_or("");
        let target = if w.0.contains_key(target) { w.of(target, &mut m) } else { m.mask(target) };
        let detail = m.mask(&detail);
        let error = m.mask(e["error"].as_str().unwrap_or(""));
        let line = format!("E{} · {} · {person} · {} · {} · {target} · {} · {}\n", eids.len(), when(e["at"].as_str().unwrap_or("")),
            e["area"].as_str().unwrap_or(""), e["action"].as_str().unwrap_or(""), detail.replace('\n', " "),
            if e["ok"] == false { format!("FAILED: {error}") } else { "ok".into() });
        // what the person said about an AI answer
        let line = match e["feedback"]["vote"].as_str() {
            Some(v) if !v.is_empty() => format!("{} · person's feedback: {}{}{}\n", line.trim_end(), if v == "up" { "helpful" } else { "NOT helpful" },
                e["feedback"]["reasons"].as_array().filter(|r| !r.is_empty()).map(|r| format!(" ({})", r.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "))).unwrap_or_default(),
                e["feedback"]["comment"].as_str().filter(|c| !c.trim().is_empty()).map(|c| format!(" “{}”", m.mask(c))).unwrap_or_default()),
            _ => line,
        };
        text.push_str(&line);
    }
    if !extra.is_empty() {
        // "{{who:login}}" in the extra facts: that person's placeholder
        let mut x = m.mask(extra);
        for (login, tok) in &w.0 {
            x = x.replace(&format!("{{{{who:{login}}}}}"), tok);
        }
        text.push_str(&format!("\n{x}\n"));
    }
    Ctx { text, masker: m, sessions: sids, events: eids }
}

/// What access can be built from: bankers, clients (placeholders only), products, groups, reports, and the people.
pub fn access_ctx(store: &Store, opts: &Json) -> (String, Masker) {
    let (mut m, w) = masker(store, opts);
    let mut text = people(store, opts, &mut m, &w);
    text.push_str("\nBANKERS (placeholder · their clients)\n");
    for b in opts["bankers"].as_array().into_iter().flatten() {
        let clients: Vec<String> = opts["clients"].as_array().into_iter().flatten().filter(|c| c["banker"] == b["name"]).map(|c| m.mask(c["name"].as_str().unwrap_or(""))).collect();
        text.push_str(&format!("{} · {} clients: {}\n", m.mask(b["name"].as_str().unwrap_or("")), clients.len(), clients.join(", ")));
    }
    text.push_str("\nPRODUCTS (code · name · clients with it)\n");
    for p in opts["sources"].as_array().into_iter().flatten() {
        text.push_str(&format!("{} · {} / {} · {}\n", p["id"].as_str().unwrap_or(""), p["name"].as_str().unwrap_or(""), p["name_en"].as_str().unwrap_or(""), p["clients"]));
    }
    text.push_str("\nGROUPS (id · name · members · AI level)\n");
    for g in &store.groups {
        let members: Vec<String> = g.members.iter().map(|l| w.of(l, &mut m)).collect();
        text.push_str(&format!("{} · {} · {} · {}\n", g.id, g.name, members.join(", "), if g.ai_level.is_empty() { "not set" } else { level_name(&g.ai_level) }));
    }
    text.push_str("\nREPORTS (id · title · published · given to)\n");
    for r in &store.reports {
        let users: Vec<String> = r.audience.users.iter().map(|l| w.of(l, &mut m)).collect();
        text.push_str(&format!("{} · {} · {} · {}{}{}\n", r.id, r.title, if r.published { "published" } else { "draft" },
            if r.audience.everyone { "everyone with the User role; " } else { "" },
            if r.audience.groups.is_empty() { String::new() } else { format!("groups {}; ", r.audience.groups.join(", ")) },
            if users.is_empty() { String::new() } else { format!("people {}", users.join(", ")) }));
    }
    (text, m)
}

/// The system prompt of a task: "ask", "explain", "access" or "review".
pub fn system(kind: &str, lang: &str) -> String {
    let ul = if lang == "ru" { "Russian" } else { "English" };
    let common = format!(
        "You assist the administrator of Quire, a private bank's report app. Identities are placeholders: [P-n] a person who uses the app (or a banker, [B-n]), \
         [CL-n] a client, [ID-n] a code; use them exactly as written, never guess what they hide. Facts only from what you are given: if it isn't there, say so. \
         Answer in {ul}. Times are local. Keep it short and concrete. Call things by the names the app uses, as given here: the AI levels are only \"Structure only\" and \"Figures\" — \
         never other names, never invented detail."
    );
    let task = match kind {
        "explain" => "TASK: explain the FAILED EVENT below: what most likely happened, why (from the event, its error and what the same person did around it), and what to do. \
            ANSWER with ONE JSON object: {\"cause\": \"one or two sentences\", \"fix\": \"what to do, as steps if useful\", \"refs\": [\"E3\", \"S2\"]}".to_string(),
        "access" => "TASK: turn the administrator's request into exact access changes. You may change people's roles (admin, data, design, user), \
            their clients (every client, or bankers' clients by banker placeholder, or clients one by one by placeholder), products (codes; empty = every product), \
            groups (ids), the reports given to them by name (ids), their AI level (\"structure\" for Structure only, \"aggregates\" for Figures, or \"\" for not set), whether they can sign in; \
            or add a new person (name, login, e-mail). Change only what is asked; keep everything else as it is. If something is ambiguous or unknown, don't guess: ask in \"questions\". \
            ANSWER with ONE JSON object: {\"explanation\": \"what you will change, in a sentence or two\", \"changes\": [{\"person\": \"[P-1]\" or \"new\", \"name\": \"only for a new person\", \"login\": \"only for a new person\", \"email\": \"\", \
            \"roles\": {\"admin\": false, \"data\": false, \"design\": false, \"user\": true} or null, \"clients\": {\"all\": false, \"bankers\": [\"[B-1]\"], \"clients\": [\"[CL-2]\"]} or null, \
            \"products\": [\"INS\"] or null, \"groups\": [\"g_bankers\"] or null, \"reports_add\": [\"uhnwi\"], \"reports_remove\": [], \"ai_level\": \"aggregates\" or null, \"active\": true or null}], \"questions\": []}\n\
            null means: leave as it is. \"groups\" is the full list the person ends up in.".to_string(),
        "review" => "TASK: review who can do what, as a careful bank security officer. Look for: more rights than a role needs (several administrators, admins who only make reports), \
            accounts still on the demo password or never used, disabled accounts still in groups or given reports, people who may send client figures to AI but never use the report assistant, \
            failed sign-ins (repeated, or for logins that don't exist), unusual activity (exports at night, one person opening many clients, very long sessions), AI use far above the others. \
            Only what the facts show; no generic advice. \
            ANSWER with ONE JSON object: {\"summary\": \"two sentences\", \"findings\": [{\"severity\": \"high|medium|low\", \"title\": \"short\", \"detail\": \"what the facts show\", \
            \"person\": \"[P-1]\" or null, \"action\": \"open|disable|ai_structure|none\", \"refs\": [\"S4\", \"E12\"]}]}".to_string(),
        _ => "TASK: answer the administrator's question about what people did, from the sessions (S…) and events (E…). Count and compare when asked; name the sessions or events your answer rests on. \
            ANSWER with ONE JSON object: {\"answer\": \"the answer; wrap the key fact in **double asterisks**\", \"refs\": [\"S3\", \"E17\"]}".to_string(),
    };
    format!("{common}\n\n{task}")
}

/// A proposed change, made exact: the person's account as it would be saved (before and after),
/// their groups and reports. Unknown people, bankers, clients, products, groups or reports are errors.
pub fn resolve_access(store: &Store, opts: &Json, answer: &Json) -> Json {
    let banker_id = |name: &str| opts["bankers"].as_array().into_iter().flatten().find(|b| b["name"] == name || b["id"] == name).and_then(|b| b["id"].as_str()).map(String::from);
    let client_id = |name: &str| opts["clients"].as_array().into_iter().flatten().find(|c| c["name"] == name || c["id"] == name).and_then(|c| c["id"].as_str()).map(String::from);
    let product_ok = |p: &str| opts["sources"].as_array().into_iter().flatten().any(|x| x["id"] == p);
    let mut out = Vec::new();
    for ch in answer["changes"].as_array().into_iter().flatten() {
        let mut problems: Vec<String> = Vec::new();
        let who = ch["person"].as_str().unwrap_or("");
        let existing = store.users.iter().find(|u| u.name == who || u.login == who || u.email == who);
        let new = existing.is_none();
        if new && who != "new" && !who.is_empty() {
            problems.push(format!("No one called “{who}”."));
        }
        let before = existing.map(|u| u.public());
        let mut after = before.clone().unwrap_or_else(|| json!({"login": "", "name": "", "email": "", "roles": {"admin": false, "data": false, "design": false, "user": true},
            "access": {"all": false, "coverage": [], "sources": [], "clients": []}, "active": true, "must_change": true, "ai_level": ""}));
        if new {
            for k in ["name", "login", "email"] {
                if let Some(v) = ch[k].as_str() { after[k] = json!(v.trim()); }
            }
            if after["login"].as_str().unwrap_or("").is_empty() { problems.push("A new person needs a login.".into()); }
        }
        if ch["roles"].is_object() {
            for r in ["admin", "data", "design", "user"] {
                if let Some(b) = ch["roles"][r].as_bool() { after["roles"][r] = json!(b); }
            }
        }
        if ch["clients"].is_object() {
            let c = &ch["clients"];
            let all = c["all"].as_bool().unwrap_or(false);
            let mut cov = Vec::new();
            for b in c["bankers"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                match banker_id(b) { Some(id) => cov.push(id), None => problems.push(format!("No banker “{b}”.")) }
            }
            let mut cl = Vec::new();
            for x in c["clients"].as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
                match client_id(x) { Some(id) => cl.push(id), None => problems.push(format!("No client “{x}”.")) }
            }
            after["access"]["all"] = json!(all);
            after["access"]["coverage"] = json!(if all { vec![] } else { cov });
            after["access"]["clients"] = json!(if all { vec![] } else { cl });
        }
        if let Some(ps) = ch["products"].as_array() {
            let ps: Vec<String> = ps.iter().filter_map(|x| x.as_str()).map(String::from).collect();
            for p in ps.iter().filter(|p| !product_ok(p)) { problems.push(format!("No product “{p}”.")); }
            after["access"]["sources"] = json!(ps);
        }
        if let Some(l) = ch["ai_level"].as_str() {
            if auth::ai_level_ok(l).is_ok() || l.is_empty() { after["ai_level"] = json!(l); } else { problems.push(format!("Unknown AI level “{l}”.")); }
        }
        if let Some(a) = ch["active"].as_bool() { after["active"] = json!(a); }
        let login = after["login"].as_str().unwrap_or("").to_string();
        let groups_before: Vec<String> = store.groups.iter().filter(|g| existing.is_some() && g.members.iter().any(|m| m == &login)).map(|g| g.id.clone()).collect();
        let groups = match ch["groups"].as_array() {
            Some(gs) => {
                let mut v = Vec::new();
                for g in gs.iter().filter_map(|x| x.as_str()) {
                    match store.groups.iter().find(|x| x.id == g || x.name == g) { Some(x) => v.push(x.id.clone()), None => problems.push(format!("No group “{g}”.")) }
                }
                v
            }
            None => groups_before.clone(),
        };
        let rep = |k: &str, problems: &mut Vec<String>| -> Vec<String> {
            ch[k].as_array().into_iter().flatten().filter_map(|x| x.as_str()).filter_map(|r| match store.reports.iter().find(|x| x.id == r || x.title == r) {
                Some(x) => Some(x.id.clone()),
                None => { problems.push(format!("No report “{r}”.")); None }
            }).collect()
        };
        let reports_add = rep("reports_add", &mut problems);
        let reports_remove = rep("reports_remove", &mut problems);
        let password = if new { temp_password() } else { String::new() };
        // the same, in words: bankers', clients' and groups' names (shown to the administrator only)
        let view = |acc: &Json, gs: &[String]| -> Json {
            let names = |ids: &Json, list: &str| -> Vec<String> {
                ids.as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(|id| opts[list].as_array().into_iter().flatten().find(|b| b["id"] == id).and_then(|b| b["name"].as_str()).unwrap_or(id).to_string()).collect()
            };
            json!({"bankers": names(&acc["coverage"], "bankers"), "clients": names(&acc["clients"], "clients"),
                "groups": gs.iter().map(|g| store.groups.iter().find(|x| &x.id == g).map(|x| x.name.clone()).unwrap_or_else(|| g.clone())).collect::<Vec<_>>()})
        };
        let view_before = before.as_ref().map(|b| view(&b["access"], &groups_before)).unwrap_or(Json::Null);
        let view_after = view(&after["access"], &groups);
        out.push(json!({"new": new, "before": before, "after": after, "groups_before": groups_before, "groups": groups, "view_before": view_before, "view_after": view_after,
            "reports_add": reports_add, "reports_remove": reports_remove, "password": password, "problems": problems}));
    }
    json!(out)
}

/// A first password for a new person (they choose their own at the first sign-in).
fn temp_password() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let n = h.finish();
    let abc = b"abcdefghjkmnpqrstuvwxyz23456789";
    (0..10).map(|i| abc[((n >> (i * 6)) as usize) % abc.len()] as char).collect()
}

/// Facts for the access review: per person, what they did in the last 30 days.
pub fn review_facts(store: &Store, sessions: &Json, events: &Json) -> String {
    let mut out = String::from("FACTS OF THE LAST 30 DAYS (per person: sessions, distinct clients, exports and at night 22:00–07:00, AI requests and tokens, sign-ins, failed sign-ins, last sign-in, demo password)\n");
    let ss: Vec<&Json> = sessions["sessions"].as_array().into_iter().flatten().collect();
    let ev: Vec<&Json> = events["rows"].as_array().into_iter().flatten().collect();
    let night = |at: &str| at.get(11..13).and_then(|h| h.parse::<u32>().ok()).is_some_and(|h| h >= 22 || h < 7);
    for u in &store.users {
        let l = u.login.as_str();
        let mine: Vec<&&Json> = ss.iter().filter(|s| s["user"] == l).collect();
        let clients: std::collections::BTreeSet<&str> = mine.iter().filter_map(|s| s["client"].as_str()).filter(|c| !c.is_empty()).collect();
        let exports: Vec<&&Json> = ev.iter().filter(|e| e["user"] == l && e["area"] == "document" && e["action"].as_str().is_some_and(|a| a.starts_with("export"))).collect();
        let ai: Vec<&&Json> = ev.iter().filter(|e| e["user"] == l && e["ai"] == true).collect();
        let signins = ev.iter().filter(|e| e["user"] == l && e["action"] == "sign-in" && e["ok"] == true).count();
        let failed = ev.iter().filter(|e| e["user"] == l && e["action"] == "sign-in" && e["ok"] == false).count();
        let last = ev.iter().find(|e| e["user"] == l && e["action"] == "sign-in" && e["ok"] == true).and_then(|e| e["at"].as_str()).map(when).unwrap_or_else(|| "never in 30 days".into());
        let longest = mine.iter().filter_map(|s| s["active_s"].as_f64()).fold(0.0, f64::max);
        out.push_str(&format!("{{{{who:{l}}}}} · sessions {} (longest {}s active) · clients {} · exports {} (at night {}) · AI {} ({} tokens) · sign-ins {signins} · failed {failed} · last {last}{}\n",
            mine.len(), longest, clients.len(), exports.len(), exports.iter().filter(|e| night(e["at"].as_str().unwrap_or(""))).count(),
            ai.len(), ai.iter().filter_map(|e| e["tokens_in"].as_u64()).sum::<u64>() + ai.iter().filter_map(|e| e["tokens_out"].as_u64()).sum::<u64>(),
            if store.users.iter().any(|x| x.login == l && x.check(auth::DEMO_PASSWORD)) { " · still on the demo password" } else { "" }));
    }
    // sign-ins tried with logins that don't exist
    let mut ghosts: std::collections::BTreeMap<&str, usize> = Default::default();
    for e in ev.iter().filter(|e| e["action"] == "sign-in" && e["ok"] == false) {
        let l = e["user"].as_str().unwrap_or("");
        if !store.users.iter().any(|u| u.login == l) { *ghosts.entry(l).or_default() += 1; }
    }
    if !ghosts.is_empty() {
        out.push_str(&format!("Failed sign-ins for logins that don't exist: {}\n", ghosts.iter().map(|(l, n)| format!("“{l}” ×{n}")).collect::<Vec<_>>().join(", ")));
    }
    out.push_str(&format!("Administrators: {}\n", store.users.iter().filter(|u| u.roles.admin && u.active).count()));
    out
}
