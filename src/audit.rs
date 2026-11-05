//! The activity log: what people do in Quire, one JSON line per event in `activity.jsonl` in
//! the app's data folder — sign-ins (failed ones too), administration, documents (new, open,
//! save, export, close), every edit, data work (SQL runs, changes written to a database) and AI
//! settings. AI requests have their own log (`ai_log.jsonl`, with what was sent and received);
//! `read` merges both, newest first, for the Admin area's activity page.
//!
//! Each event: `at` (local time), `user`, `area` (auth, admin, document, edit, data, ai),
//! `action`, `target` (a document, a person, a group…), `detail` (one line for people) and
//! optional `data` (the request itself, for root-cause analysis), `ok` and `error`. Passwords and
//! keys are never written.

use serde_json::{json, Value as Json};
use std::io::Write as _;
use std::path::Path;

const FILE: &str = "activity.jsonl";
/// The file is cut back to its newer half past this size.
const MAX_BYTES: u64 = 40 * 1024 * 1024;

#[allow(clippy::too_many_arguments)]
pub fn record(dir: Option<&Path>, user: &str, area: &str, action: &str, target: &str, detail: &str, data: Json, error: Option<&str>) {
    record_in(dir, "", user, area, action, target, detail, data, error);
}

/// `record`, as part of a work session (`session`: its id; "" for none).
#[allow(clippy::too_many_arguments)]
pub fn record_in(dir: Option<&Path>, session: &str, user: &str, area: &str, action: &str, target: &str, detail: &str, data: Json, error: Option<&str>) {
    let Some(dir) = dir else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join(FILE);
    trim(&path);
    let mut e = json!({"at": chrono::Local::now().to_rfc3339(), "user": user, "area": area, "action": action, "target": target, "detail": detail, "ok": error.is_none()});
    if !data.is_null() {
        // a long request stays readable but bounded
        let s = data.to_string();
        e["data"] = if s.len() > 20_000 { json!(format!("{}…", s.chars().take(20_000).collect::<String>())) } else { data };
    }
    if let Some(err) = error {
        e["error"] = json!(err.chars().take(2000).collect::<String>());
    }
    if !session.is_empty() {
        e["session"] = json!(session);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{e}");
    }
}

fn trim(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else { return };
    if meta.len() < MAX_BYTES {
        return;
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = text.lines().collect();
        let keep = lines[lines.len() / 2..].join("\n");
        let _ = std::fs::write(path, keep + "\n");
    }
}

/// Which events to read: by person, area, result, time (RFC 3339 prefix compare) and words.
#[derive(serde::Deserialize, Default, Debug)]
#[serde(default)]
pub struct Filter {
    pub user: String,
    pub area: String,
    /// "ok", "failed" or "" (all)
    pub result: String,
    pub from: String,
    pub to: String,
    pub q: String,
    pub limit: usize,
}

/// People's thumbs up or down on AI answers (`ai_feedback.jsonl` beside the AI log): the latest
/// vote of each person on each request, by (request time, person).
pub fn feedback(ai_dir: Option<&Path>) -> std::collections::HashMap<(String, String), Json> {
    let mut out = std::collections::HashMap::new();
    let Some(d) = ai_dir else { return out };
    for f in std::fs::read_to_string(d.join("ai_feedback.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()) {
        let key = (f["request_at"].as_str().unwrap_or("").to_string(), f["user"].as_str().unwrap_or("").to_string());
        out.insert(key, json!({"vote": f["vote"], "reasons": f["reasons"], "comment": f["comment"], "at": f["at"]}));
    }
    out
}

/// Writes a vote on an AI answer.
pub fn record_feedback(ai_dir: &Path, user: &str, request_at: &str, feature: &str, vote: &str, reasons: &[String], comment: &str) -> Result<(), String> {
    std::fs::create_dir_all(ai_dir).map_err(|e| e.to_string())?;
    let line = json!({"at": chrono::Local::now().to_rfc3339(), "user": user, "request_at": request_at, "feature": feature, "vote": vote, "reasons": reasons, "comment": comment.chars().take(2000).collect::<String>()});
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(ai_dir.join("ai_feedback.jsonl")).map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())
}

/// The activity, newest first, with the AI requests from `ai_dir` in it (as area "ai").
pub fn read(dir: Option<&Path>, ai_dir: Option<&Path>, f: &Filter) -> Json {
    let mut all: Vec<Json> = Vec::new();
    if let Some(d) = dir {
        let text = std::fs::read_to_string(d.join(FILE)).unwrap_or_default();
        all.extend(text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()));
    }
    let votes = feedback(ai_dir);
    if let Some(d) = ai_dir {
        let text = std::fs::read_to_string(d.join("ai_log.jsonl")).unwrap_or_default();
        for l in text.lines() {
            let Ok(a) = serde_json::from_str::<Json>(l) else { continue };
            let feature = a["feature"].as_str().unwrap_or("").to_string();
            let tokens = match (a["tokens_in"].as_u64(), a["tokens_out"].as_u64()) {
                (Some(i), Some(o)) => format!(" · {i} → {o} tokens"),
                _ => String::new(),
            };
            all.push(json!({
                "at": a["at"], "user": a["user"], "area": "ai", "action": feature,
                "target": format!("{} · {}", a["provider"].as_str().unwrap_or(""), a["model"].as_str().unwrap_or("")),
                "detail": format!("{}{tokens}{}", a["prompt"].as_str().filter(|p| !p.is_empty()).map(|p| format!("“{}”", p.chars().take(120).collect::<String>())).unwrap_or_else(|| "AI request".into()),
                    a["ms"].as_u64().map(|ms| format!(" · {:.1} s", ms as f64 / 1000.0)).unwrap_or_default()),
                "ok": a["ok"], "error": a["error"], "ai": true, "session": a["session"],
                "tokens_in": a["tokens_in"], "tokens_out": a["tokens_out"], "key": a["key"],
                "feedback": votes.get(&(a["at"].as_str().unwrap_or("").to_string(), a["user"].as_str().unwrap_or("").to_string())).cloned().unwrap_or(Json::Null),
            }));
        }
    }
    all.retain(|e| !(e["area"] == "session" && (e["action"] == "time" || e["action"] == "rebuilt")));
    all.sort_by(|a, b| b["at"].as_str().unwrap_or("").cmp(a["at"].as_str().unwrap_or("")));
    let q = f.q.to_lowercase();
    let users: std::collections::BTreeSet<String> = all.iter().filter_map(|e| e["user"].as_str().map(String::from)).filter(|u| !u.is_empty()).collect();
    let total = all.len();
    let rows: Vec<Json> = all
        .into_iter()
        .filter(|e| f.user.is_empty() || e["user"] == f.user.as_str())
        .filter(|e| f.area.is_empty() || e["area"] == f.area.as_str())
        .filter(|e| match f.result.as_str() { "ok" => e["ok"] == true, "failed" => e["ok"] == false, "unhelpful" => e["feedback"]["vote"] == "down", "helpful" => e["feedback"]["vote"] == "up", _ => true })
        .filter(|e| f.from.is_empty() || e["at"].as_str().unwrap_or("") >= f.from.as_str())
        .filter(|e| f.to.is_empty() || e["at"].as_str().unwrap_or("") <= format!("{}\u{ffff}", f.to).as_str())
        .filter(|e| q.is_empty() || e.to_string().to_lowercase().contains(&q))
        .take(if f.limit == 0 { 2000 } else { f.limit })
        .collect();
    json!({"rows": rows, "users": users, "total": total})
}

/// Work sessions, newest first: one per document and client, from opening it until another
/// client, closing it, signing out or quitting — with its context (template, client, period,
/// parameters), time (wall and active), and what happened in it (edits with their changes,
/// saves, exports, AI requests with their tokens). Each carries its events, oldest first.
pub fn sessions(dir: Option<&Path>, ai_dir: Option<&Path>, f: &Filter) -> Json {
    use std::collections::BTreeMap;
    let mut by: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    if let Some(d) = dir {
        let text = std::fs::read_to_string(d.join(FILE)).unwrap_or_default();
        for e in text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()) {
            if let Some(s) = e["session"].as_str().filter(|s| !s.is_empty()) {
                by.entry(s.to_string()).or_default().push(e);
            }
        }
    }
    let votes = feedback(ai_dir);
    if let Some(d) = ai_dir {
        let text = std::fs::read_to_string(d.join("ai_log.jsonl")).unwrap_or_default();
        for a in text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()) {
            if let Some(s) = a["session"].as_str().filter(|s| !s.is_empty()) {
                by.entry(s.to_string()).or_default().push(json!({
                    "at": a["at"], "user": a["user"], "area": "ai", "action": a["feature"], "ai": true, "ok": a["ok"], "error": a["error"],
                    "target": format!("{} · {}", a["provider"].as_str().unwrap_or(""), a["model"].as_str().unwrap_or("")),
                    "detail": a["prompt"], "tokens_in": a["tokens_in"], "tokens_out": a["tokens_out"], "ms": a["ms"], "key": a["key"],
                    "feedback": votes.get(&(a["at"].as_str().unwrap_or("").to_string(), a["user"].as_str().unwrap_or("").to_string())).cloned().unwrap_or(Json::Null),
                }));
            }
        }
    }
    let q = f.q.to_lowercase();
    let mut out: Vec<Json> = Vec::new();
    let mut users = std::collections::BTreeSet::new();
    for (id, mut ev) in by {
        ev.sort_by(|a, b| a["at"].as_str().unwrap_or("").cmp(b["at"].as_str().unwrap_or("")));
        let user = ev.iter().find_map(|e| e["user"].as_str().filter(|u| !u.is_empty())).unwrap_or("").to_string();
        // the context: the last "start" or "context" line (a client picked right after opening updates it)
        let ctx = ev.iter().rev().find(|e| e["area"] == "session" && (e["action"] == "start" || e["action"] == "context")).map(|e| e["data"].clone()).unwrap_or(Json::Null);
        let start = ev.first().and_then(|e| e["at"].as_str()).unwrap_or("").to_string();
        let end_ev = ev.iter().rev().find(|e| e["area"] == "session" && e["action"] == "end");
        let last = ev.last().and_then(|e| e["at"].as_str()).unwrap_or("").to_string();
        let end = end_ev.and_then(|e| e["data"]["last"].as_str().or(e["at"].as_str())).map(String::from).unwrap_or_else(|| last.clone());
        // active time: the end line's, else the last time-keeping line's (an app that quit without saying so)
        let active = ev.iter().filter(|e| e["area"] == "session").filter_map(|e| e["data"]["active_s"].as_f64()).fold(0.0_f64, f64::max);
        let wall = match (chrono::DateTime::parse_from_rfc3339(&start), chrono::DateTime::parse_from_rfc3339(&end)) {
            (Ok(a), Ok(b)) => (b - a).num_seconds().max(0) as f64,
            _ => 0.0,
        };
        let work: Vec<&Json> = ev.iter().filter(|e| e["area"] != "session").collect();
        // changes: a document's edits, or an administrator's (people, groups, audiences, AI policy and keys)
        let edits = work.iter().filter(|e| (e["area"] == "edit" && e["action"] != "undo" && e["action"] != "redo") || e["area"] == "admin" || (e["area"] == "ai" && e["ai"] != true)).count();
        let exports: Vec<Json> = work.iter().filter(|e| e["area"] == "document" && e["action"] == "export" || e["action"] == "export brief").map(|e| json!({"at": e["at"], "detail": e["detail"], "ok": e["ok"]})).collect();
        let saves = work.iter().filter(|e| e["area"] == "document" && e["action"] == "save").count();
        let ai: Vec<&&Json> = work.iter().filter(|e| e["ai"] == true).collect();
        let tok = |k: &str| ai.iter().filter_map(|e| e[k].as_u64()).sum::<u64>();
        let failed = work.iter().filter(|e| e["ok"] == false).count();
        let rebuilt = ev.iter().any(|e| e["data"]["rebuilt"] == true);
        let s = json!({
            "id": id, "user": user, "rebuilt": rebuilt, "start": start, "end": end, "open": end_ev.is_none(), "reason": end_ev.map(|e| e["data"]["reason"].clone()).unwrap_or(Json::Null),
            "active_s": if rebuilt { Json::Null } else { json!(active.round()) }, "wall_s": wall,
            "kind": ctx["kind"], "title": ctx["title"], "template": ctx["template"], "client": ctx["client"], "period": ctx["period"], "params": ctx["params"],
            "edits": edits, "saves": saves, "exports": exports, "ai": ai.len(), "tokens_in": tok("tokens_in"), "tokens_out": tok("tokens_out"), "failed": failed,
            "events": ev,
        });
        users.insert(user.clone());
        let keep = (f.user.is_empty() || user == f.user)
            && (f.from.is_empty() || end.as_str() >= f.from.as_str())
            && (f.to.is_empty() || start.as_str() <= format!("{}\u{ffff}", f.to).as_str())
            && (f.result != "failed" || failed > 0)
            && (q.is_empty() || s.to_string().to_lowercase().contains(&q));
        if keep {
            out.push(s);
        }
    }
    out.sort_by(|a, b| b["start"].as_str().unwrap_or("").cmp(a["start"].as_str().unwrap_or("")));
    out.truncate(if f.limit == 0 { 500 } else { f.limit });
    json!({"sessions": out, "users": users})
}

/// What a change changed: each leaf the patch sets, with the value it had (paths like
/// "opts.table.row_h"; long values cut short).
pub fn changes(before: &Json, patch: &Json) -> Vec<Json> {
    fn short(v: &Json) -> Json {
        let s = if v.is_string() { v.as_str().unwrap_or("").to_string() } else { v.to_string() };
        if s.chars().count() > 160 { json!(format!("{}…", s.chars().take(160).collect::<String>())) } else if v.is_string() || v.is_null() { v.clone() } else { json!(s) }
    }
    fn walk(path: &str, before: &Json, patch: &Json, out: &mut Vec<Json>) {
        match patch.as_object() {
            Some(o) if !o.is_empty() && before.is_object() => {
                for (k, v) in o {
                    walk(&if path.is_empty() { k.clone() } else { format!("{path}.{k}") }, &before[k], v, out);
                }
            }
            _ if before != patch => out.push(json!({"path": path, "from": short(before), "to": short(patch)})),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk("", before, patch, &mut out);
    out.truncate(40);
    out
}

/// One line about an edit, for people: what kind of change, on which widget or page.
pub fn edit_line(op: &Json, doc: Option<&crate::model::Doc>) -> (String, String) {
    let kind = op["op"].as_str().unwrap_or("edit").to_string();
    let name_of = |id: &str| -> String {
        doc.and_then(|d| d.find_widget(id).map(|(pi, wi)| {
            let w = &d.pages[pi].widgets[wi];
            let n = w.opt_str("name").filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| if w.title.is_empty() { crate::vm::kind_name(&w.kind).to_string() } else { w.title.clone() });
            format!("“{n}” on page {}", pi + 1)
        })).unwrap_or_else(|| id.to_string())
    };
    let page_of = |id: &str| -> String {
        doc.and_then(|d| d.page_index(id).map(|i| format!("page {} “{}”", i + 1, d.pages[i].title))).unwrap_or_else(|| id.to_string())
    };
    let what = if let Some(id) = op["id"].as_str() {
        let keys: Vec<String> = op["patch"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        format!("{}{}", name_of(id), if keys.is_empty() { String::new() } else { format!(" · {}", keys.join(", ")) })
    } else if let Some(p) = op["page"].as_str() {
        page_of(p)
    } else if let Some(n) = op["dataset"]["name"].as_str() {
        format!("source “{n}”")
    } else if let Some(k) = op["patch"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")) {
        k
    } else {
        String::new()
    };
    (kind, what)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_written_and_read_back_filtered_with_ai_requests() {
        let dir = std::env::temp_dir().join(format!("quire_audit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        record(Some(&dir), "admin", "auth", "sign-in", "admin", "Signed in", Json::Null, None);
        record(Some(&dir), "eve", "auth", "sign-in", "eve", "Sign-in failed", Json::Null, Some("Wrong login or password."));
        record(Some(&dir), "admin", "edit", "update_widget", "Отчёт", "“Total” on page 3 · title", json!({"op": "update_widget"}), None);
        std::fs::write(dir.join("ai_log.jsonl"), json!({"at": "2099-01-01T00:00:00+00:00", "user": "user", "feature": "report.note", "provider": "claude", "model": "m", "ok": true, "tokens_in": 10, "tokens_out": 5, "prompt": "", "ms": 5000}).to_string() + "\n").unwrap();
        let all = read(Some(&dir), Some(&dir), &Filter::default());
        assert_eq!(all["total"], 4);
        assert_eq!(all["rows"][0]["area"], "ai", "newest first, AI requests in it");
        assert!(all["rows"][0]["detail"].as_str().unwrap().contains("10 → 5 tokens"));
        let failed = read(Some(&dir), None, &Filter { result: "failed".into(), ..Default::default() });
        assert_eq!(failed["rows"].as_array().unwrap().len(), 1);
        assert_eq!(failed["rows"][0]["error"], "Wrong login or password.");
        // a vote on the AI request: on its row, and what "not helpful" finds
        record_feedback(&dir, "user", "2099-01-01T00:00:00+00:00", "report.note", "down", &["Not useful".into()], "too generic").unwrap();
        let unhelpful = read(Some(&dir), Some(&dir), &Filter { result: "unhelpful".into(), ..Default::default() });
        assert_eq!(unhelpful["rows"].as_array().unwrap().len(), 1);
        assert_eq!(unhelpful["rows"][0]["feedback"]["comment"], "too generic");
        record_feedback(&dir, "user", "2099-01-01T00:00:00+00:00", "report.note", "up", &[], "").unwrap();
        assert_eq!(read(Some(&dir), Some(&dir), &Filter { result: "unhelpful".into(), ..Default::default() })["rows"].as_array().unwrap().len(), 0, "the latest vote counts");
        let words = read(Some(&dir), None, &Filter { q: "total".into(), ..Default::default() });
        assert_eq!(words["rows"].as_array().unwrap().len(), 1);
        assert!(!std::fs::read_to_string(dir.join(FILE)).unwrap().contains("password\":"), "no passwords");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Gives sessions to the events logged before sessions existed (once): each person's work grouped
/// by document and client — a new one when they open or create a document, switch its client, or
/// after 30 minutes without anything; signing out or closing ends it. Administration becomes admin
/// sessions; AI requests join the session open at the time. Rebuilt sessions say so and have no
/// active time (it wasn't measured). The log files are copied aside first (`*.before-sessions`).
/// Answers how many sessions were made.
pub fn rebuild_once(dir: &Path, ai_dir: Option<&Path>) -> usize {
    let path = dir.join(FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return 0 };
    let mut acts: Vec<Json> = text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()).collect();
    if acts.iter().any(|e| e["area"] == "session" && e["action"] == "rebuilt") {
        return 0;
    }
    let ai_path = ai_dir.map(|d| d.join("ai_log.jsonl"));
    let mut ais: Vec<Json> = ai_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()).collect()).unwrap_or_default();
    // the order things happened in: (time, which list, index)
    let mut order: Vec<(String, bool, usize)> = acts.iter().enumerate().map(|(i, e)| (e["at"].as_str().unwrap_or("").to_string(), false, i))
        .chain(ais.iter().enumerate().map(|(i, e)| (e["at"].as_str().unwrap_or("").to_string(), true, i))).collect();
    order.sort();
    struct Open { id: String, title: String, client: String, kind: String, first: String, last: String, admin: bool }
    let mut made: Vec<(Open, String)> = Vec::new();
    let mut doc: std::collections::HashMap<String, Open> = Default::default();
    let mut adm: std::collections::HashMap<String, Open> = Default::default();
    let mut n = 0u32;
    let gap = |a: &str, b: &str| match (chrono::DateTime::parse_from_rfc3339(a), chrono::DateTime::parse_from_rfc3339(b)) {
        (Ok(x), Ok(y)) => (y - x).num_minutes() >= 30,
        _ => false,
    };
    let mut new_id = |at: &str| { n += 1; format!("{}-r{n}", at.get(..19).unwrap_or("").replace(['-', ':', 'T'], "")) };
    // the client a line names: a parameter change, or a brief's file name ("Brief — Client — Period")
    let client_of = |e: &Json| -> Option<String> {
        // an edit's request: wrapped ({"op": …}) since edits record their changes, bare before that
        for c in [&e["data"]["op"]["params"]["values"]["client"], &e["data"]["params"]["values"]["client"]] {
            if let Some(c) = c.as_str().filter(|c| !c.is_empty() && *c != "All") { return Some(c.to_string()); }
        }
        let d = e["detail"].as_str().unwrap_or("");
        d.split_once("Brief — ").or_else(|| d.split_once("Бриф — ")).and_then(|(_, rest)| rest.split(" — ").next()).map(|c| c.trim().to_string()).filter(|c| !c.is_empty())
    };
    for (at, is_ai, i) in order {
        let e = if is_ai { &ais[i] } else { &acts[i] };
        if e["session"].as_str().is_some_and(|s| !s.is_empty()) {
            continue;
        }
        let user = e["user"].as_str().unwrap_or("").to_string();
        if user.is_empty() {
            continue;
        }
        let area = e["area"].as_str().unwrap_or("");
        let action = e["action"].as_str().unwrap_or("");
        // signing in or out: whatever was open ended
        if !is_ai && area == "auth" && (action == "sign-in" || action == "sign-out") {
            let why = if action == "sign-out" { "signed out" } else { "signed in again" };
            if let Some(o) = doc.remove(&user) { made.push((o, why.into())); }
            if let Some(o) = adm.remove(&user) { made.push((o, why.into())); }
            continue;
        }
        let feature = e["feature"].as_str().unwrap_or("");
        let admin_work = if is_ai { feature.starts_with("admin.") } else { area == "admin" || (area == "ai" && matches!(action, "bank policy" | "save key" | "remove key")) };
        let sid = if admin_work {
            let stale = adm.get(&user).is_some_and(|o| gap(&o.last, &at));
            if stale { let o = adm.remove(&user).unwrap(); made.push((o, "left".into())); }
            let o = adm.entry(user.clone()).or_insert_with(|| Open { id: new_id(&at), title: "Administration".into(), client: String::new(), kind: "admin".into(), first: at.clone(), last: at.clone(), admin: true });
            o.last = at.clone();
            o.id.clone()
        } else if is_ai {
            // an assistant's request: the document session open then
            match doc.get_mut(&user).filter(|o| !gap(&o.last, &at)) {
                Some(o) if !feature.starts_with("sql.") || !o.title.is_empty() => { o.last = at.clone(); o.id.clone() }
                _ => continue,
            }
        } else if matches!(area, "data" | "ai") {
            // data work and AI keys belong to a document only while one is open (as they do now)
            match doc.get_mut(&user).filter(|o| !gap(&o.last, &at)) {
                Some(o) => { o.last = at.clone(); o.id.clone() }
                None => continue,
            }
        } else if matches!(area, "document" | "edit") {
            let title = e["target"].as_str().unwrap_or("").to_string();
            let opens = area == "document" && matches!(action, "new" | "open" | "new report");
            let client = client_of(e);
            let cur = doc.get(&user);
            let restart = opens || cur.is_none() || cur.is_some_and(|o| gap(&o.last, &at) || (!title.is_empty() && o.title != title)
                || client.as_ref().is_some_and(|c| !o.client.is_empty() && &o.client != c));
            if restart {
                let why = match cur {
                    Some(o) if client.as_ref().is_some_and(|c| !o.client.is_empty() && &o.client != c) => "another client",
                    Some(o) if gap(&o.last, &at) => "idle",
                    _ => "another document",
                };
                if let Some(o) = doc.remove(&user) { made.push((o, why.into())); }
                let d = e["detail"].as_str().unwrap_or("");
                let kind = if d.contains("(pub:") || action == "new report" { "report" } else if d.contains("(uhnwi_report") || d.contains("template") { "template" } else { "report" };
                doc.insert(user.clone(), Open { id: new_id(&at), title: title.clone(), client: client.clone().unwrap_or_default(), kind: kind.into(), first: at.clone(), last: at.clone(), admin: false });
            }
            let o = doc.get_mut(&user).unwrap();
            o.last = at.clone();
            if o.client.is_empty() { if let Some(c) = client { o.client = c; } }
            let id = o.id.clone();
            if area == "document" && action == "close" {
                let o = doc.remove(&user).unwrap();
                made.push((o, "closed".into()));
            }
            id
        } else {
            continue;
        };
        if is_ai { ais[i]["session"] = json!(sid); } else { acts[i]["session"] = json!(sid); }
    }
    for (_, o) in doc.drain().chain(adm.drain()) {
        made.push((o, "rebuilt".into()));
    }
    if made.is_empty() {
        // nothing to rebuild: just remember it was looked at
        acts.push(json!({"at": chrono::Local::now().to_rfc3339(), "user": "", "area": "session", "action": "rebuilt", "target": "", "detail": "", "ok": true}));
    } else {
        let _ = std::fs::copy(&path, dir.join(format!("{FILE}.before-sessions")));
        if let Some(p) = &ai_path { let _ = std::fs::copy(p, p.with_extension("jsonl.before-sessions")); }
        for (o, why) in &made {
            let user = acts.iter().chain(ais.iter()).find(|e| e["session"] == o.id.as_str()).and_then(|e| e["user"].as_str()).unwrap_or("").to_string();
            let detail = if o.admin { "Administration".to_string() } else { format!("{} · {}", o.title, if o.client.is_empty() { "client not recorded" } else { &o.client }) };
            acts.push(json!({"at": o.first, "user": user, "area": "session", "action": "start", "target": o.title, "detail": detail, "ok": true, "session": o.id,
                "data": {"kind": o.kind, "title": o.title, "client": o.client, "rebuilt": true}}));
            acts.push(json!({"at": o.last, "user": user, "area": "session", "action": "end", "target": "", "detail": "rebuilt from the log", "ok": true, "session": o.id,
                "data": {"reason": why, "rebuilt": true}}));
        }
        // at the same moment a session starts before what happens in it and ends after
        let rank = |e: &Json| if e["area"] == "session" { if e["action"] == "start" { 0 } else { 2 } } else { 1 };
        acts.sort_by(|a, b| a["at"].as_str().unwrap_or("").cmp(b["at"].as_str().unwrap_or("")).then(rank(a).cmp(&rank(b))));
        acts.push(json!({"at": chrono::Local::now().to_rfc3339(), "user": "", "area": "session", "action": "rebuilt", "target": "", "detail": format!("{} older sessions rebuilt", made.len()), "ok": true}));
        if let Some(p) = &ai_path {
            let body: String = ais.iter().map(|e| format!("{e}\n")).collect();
            let _ = std::fs::write(p, body);
        }
    }
    let body: String = acts.iter().map(|e| format!("{e}\n")).collect();
    let _ = std::fs::write(&path, body);
    made.len()
}

/// A person's own reports per client: when they last worked on one, and last exported one
/// (from the sessions' context lines and their exports). Kept between calls: each call reads only
/// what the log gained since (from scratch when the log was trimmed).
pub fn my_reports(dir: Option<&Path>, user: &str) -> Json {
    #[derive(Default)]
    struct Index {
        read: u64,
        client_of: std::collections::HashMap<String, String>,
        out: serde_json::Map<String, Json>,
    }
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<(std::path::PathBuf, String), Index>>> = std::sync::OnceLock::new();
    let Some(d) = dir else { return json!({}) };
    let path = d.join(FILE);
    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut all = match CACHE.get_or_init(Default::default).lock() { Ok(g) => g, Err(e) => e.into_inner() };
    let ix = all.entry((path.clone(), user.to_string())).or_default();
    if len < ix.read {
        *ix = Index::default();
    }
    if len > ix.read {
        use std::io::{Read, Seek, SeekFrom};
        let mut buf = Vec::new();
        if let Ok(mut f) = std::fs::File::open(&path) {
            if f.seek(SeekFrom::Start(ix.read)).is_ok() {
                let _ = f.read_to_end(&mut buf);
            }
        }
        // whole lines only: a line still being written waits for the next call
        let upto = buf.iter().rposition(|&c| c == b'\n').map(|i| i + 1).unwrap_or(0);
        ix.read += upto as u64;
        let text = String::from_utf8_lossy(&buf[..upto]);
        for e in text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()) {
            if e["user"] != user {
                continue;
            }
            let out = &mut ix.out;
            // a report made in a batch: exported for its client
            if e["area"] == "document" && e["action"] == "batch export" && e["ok"] != false {
                if let (Some(c), Some(at)) = (e["data"]["client"].as_str(), e["at"].as_str()) {
                    let slot = out.entry(c.to_string()).or_insert_with(|| json!({ "opened": Json::Null, "exported": Json::Null }));
                    slot["exported"] = json!(at);
                    if slot["opened"].is_null() { slot["opened"] = json!(at); }
                }
                continue;
            }
            let (Some(s), Some(at)) = (e["session"].as_str(), e["at"].as_str()) else { continue };
            if e["area"] == "session" && (e["action"] == "start" || e["action"] == "context") && e["data"]["kind"] == "report" {
                if let Some(c) = e["data"]["client"].as_str().filter(|c| !c.is_empty()) {
                    ix.client_of.insert(s.to_string(), c.to_string());
                }
            }
            let Some(c) = ix.client_of.get(s) else { continue };
            let slot = out.entry(c.clone()).or_insert_with(|| json!({ "opened": Json::Null, "exported": Json::Null }));
            if e["area"] != "session" || e["action"] == "start" {
                slot["opened"] = json!(at);
            }
            if e["area"] == "document" && e["action"] == "export" && e["ok"] != false {
                slot["exported"] = json!(at);
            }
        }
    }
    Json::Object(ix.out.clone())
}

/// For the designer's start page: how each published report was used since `since` (reports
/// opened from it, exported, by how many people, for how many clients, last used) — keyed by the
/// report's id and by its title — and the template files `user` opened or saved, newest first.
pub fn design_usage(dir: Option<&Path>, user: &str, since: &str) -> Json {
    let Some(d) = dir else { return json!({"usage": {}, "recent": []}) };
    let text = std::fs::read_to_string(d.join(FILE)).unwrap_or_default();
    #[derive(Default)]
    struct U { opened: u64, exported: u64, people: std::collections::BTreeSet<String>, clients: std::collections::BTreeSet<String>, last: String }
    let mut usage: std::collections::BTreeMap<String, U> = Default::default();
    let mut session_of: std::collections::HashMap<String, (Vec<String>, String)> = Default::default();
    let mut recent: Vec<(String, String)> = Vec::new();
    for e in text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()) {
        let at = e["at"].as_str().unwrap_or("").to_string();
        let who = e["user"].as_str().unwrap_or("").to_string();
        // a template file this person opened or saved
        if who == user && e["area"] == "document" && (e["action"] == "open" || e["action"] == "save") && e["ok"] != false {
            if let Some(p) = e["detail"].as_str().filter(|p| p.ends_with(".rtpl")) {
                recent.retain(|(q, _)| q != p);
                recent.push((p.to_string(), at.clone()));
            }
        }
        if at.as_str() < since {
            continue;
        }
        if e["area"] == "session" && (e["action"] == "start" || e["action"] == "context") && e["data"]["kind"] == "report" {
            let keys: Vec<String> = [&e["data"]["report"], &e["data"]["template"]].iter().filter_map(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from).collect();
            if keys.is_empty() { continue; }
            let client = e["data"]["client"].as_str().unwrap_or("").to_string();
            for k in &keys {
                let u = usage.entry(k.clone()).or_default();
                u.opened += 1;
                u.people.insert(who.clone());
                if !client.is_empty() { u.clients.insert(client.clone()); }
                if at > u.last { u.last = at.clone(); }
            }
            if let Some(s) = e["session"].as_str() { session_of.insert(s.to_string(), (keys, client)); }
            continue;
        }
        let exported = e["area"] == "document" && e["ok"] != false && (e["action"] == "export" || e["action"] == "batch export");
        if !exported { continue; }
        let keys: Vec<String> = if e["action"] == "batch export" {
            e["target"].as_str().filter(|s| !s.is_empty()).map(|s| vec![s.to_string()]).unwrap_or_default()
        } else {
            e["session"].as_str().and_then(|s| session_of.get(s)).map(|(k, _)| k.clone()).unwrap_or_default()
        };
        for k in keys {
            let u = usage.entry(k).or_default();
            u.exported += 1;
            u.people.insert(who.clone());
            if let Some(c) = e["data"]["client"].as_str().filter(|c| !c.is_empty()) { u.clients.insert(c.to_string()); }
            if at > u.last { u.last = at.clone(); }
        }
    }
    let usage: serde_json::Map<String, Json> = usage.into_iter().map(|(k, u)| (k, json!({"opened": u.opened, "exported": u.exported, "people": u.people.len(), "clients": u.clients.len(), "last": u.last}))).collect();
    let recent: Vec<Json> = recent.into_iter().rev().filter(|(p, _)| Path::new(p).exists()).take(8).map(|(p, at)| json!({"path": p, "at": at})).collect();
    json!({"usage": usage, "recent": recent})
}

#[cfg(test)]
mod design_usage_tests {
    use super::*;

    #[test]
    fn reports_count_their_use_and_templates_come_back_newest_first() {
        let dir = std::env::temp_dir().join(format!("quire_dusage_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tpl = dir.join("a.rtpl");
        std::fs::write(&tpl, "{}").unwrap();
        let ev = |j: Json| j.to_string() + "\n";
        let mut log = String::new();
        log += &ev(json!({"user": "d", "area": "document", "action": "open", "detail": tpl.display().to_string(), "at": "2026-10-01T09:00:00", "ok": true}));
        log += &ev(json!({"user": "b1", "session": "s1", "area": "session", "action": "start", "at": "2026-10-20T10:00:00", "ok": true, "data": {"kind": "report", "report": "uhnwi", "template": "Отчёт", "client": "A"}}));
        log += &ev(json!({"user": "b1", "session": "s1", "area": "document", "action": "export", "at": "2026-10-20T10:05:00", "ok": true}));
        log += &ev(json!({"user": "b2", "session": "s2", "area": "session", "action": "start", "at": "2026-10-21T10:00:00", "ok": true, "data": {"kind": "report", "report": "uhnwi", "client": "B"}}));
        log += &ev(json!({"user": "b2", "area": "document", "action": "batch export", "target": "Отчёт", "at": "2026-10-22T10:00:00", "ok": true, "data": {"client": "C"}}));
        // before the window: not counted
        log += &ev(json!({"user": "b3", "session": "s0", "area": "session", "action": "start", "at": "2026-08-01T10:00:00", "ok": true, "data": {"kind": "report", "report": "uhnwi", "client": "Z"}}));
        std::fs::write(dir.join(FILE), log).unwrap();
        let r = design_usage(Some(&dir), "d", "2026-09-24");
        assert_eq!(r["usage"]["uhnwi"], json!({"opened": 2, "exported": 1, "people": 2, "clients": 2, "last": "2026-10-21T10:00:00"}));
        assert_eq!(r["usage"]["Отчёт"]["exported"], 2);
        assert_eq!(r["recent"][0]["path"], tpl.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod my_reports_tests {
    use super::*;

    /// The index reads only what the log gained, and starts over when the log is trimmed.
    #[test]
    fn last_reports_read_only_what_is_new() {
        let dir = std::env::temp_dir().join(format!("quire_myrep_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let line = |s: &str, area: &str, action: &str, at: &str, client: &str| json!({"user": "u", "session": s, "area": area, "action": action, "at": at, "ok": true,
            "data": {"kind": "report", "client": client}}).to_string() + "\n";
        let f = dir.join(FILE);
        std::fs::write(&f, line("s1", "session", "start", "2026-10-01T10:00:00", "A")).unwrap();
        assert_eq!(my_reports(Some(&dir), "u")["A"]["opened"], "2026-10-01T10:00:00");
        // a new line: read on top of what was read
        let mut more = std::fs::read_to_string(&f).unwrap();
        more += &line("s1", "document", "export", "2026-10-02T10:00:00", "A");
        std::fs::write(&f, more).unwrap();
        let r = my_reports(Some(&dir), "u");
        assert_eq!((r["A"]["opened"].as_str(), r["A"]["exported"].as_str()), (Some("2026-10-02T10:00:00"), Some("2026-10-02T10:00:00")));
        // trimmed (shorter): from scratch
        std::fs::write(&f, line("s2", "session", "start", "2026-10-03T10:00:00", "B")).unwrap();
        let r = my_reports(Some(&dir), "u");
        assert!(r.get("A").is_none() && r["B"]["opened"] == "2026-10-03T10:00:00", "{r}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
