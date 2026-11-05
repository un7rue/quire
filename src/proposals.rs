//! The register of proposals: every proposal a banker puts in a report, kept by Quire beside the
//! report itself — who made it, for which client, report and period, its content and notes, where
//! it came from (notes, an AI write-up, an AI draft), each earlier version, and its status
//! (draft → presented → accepted / declined / partly; withdrawn) with that status's history.
//! Bankers see their clients' proposals (in the client list, the proposal window and the report's
//! AI); administrators see all of them for support. Kept as `proposals.json` beside the accounts.

use serde_json::{json, Value as Json};
use std::path::Path;

const FILE: &str = "proposals.json";
/// The statuses a proposal can have, in their usual order.
pub const STATUSES: [&str; 6] = ["draft", "presented", "accepted", "partly", "declined", "withdrawn"];
/// A presented proposal with no outcome after this many days is a reminder in "Needs attention".
pub const FOLLOW_UP_DAYS: i64 = 14;
const MAX_VERSIONS: usize = 30;

fn load(dir: &Path) -> Vec<Json> {
    std::fs::read_to_string(dir.join(FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(dir: &Path, all: &[Json]) -> Result<(), String> {
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(FILE)).map_err(|e| e.to_string())
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

/// A new register id.
pub fn new_uid() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    format!("P{}-{:06x}", chrono::Local::now().format("%Y%m%d%H%M%S"), h.finish() & 0xff_ffff)
}

/// What a proposal says, as the register keeps it.
fn content(spec: &Json) -> Json {
    let pick = |k: &str| spec.get(k).cloned().unwrap_or(Json::Null);
    json!({"title": pick("title"), "lead": pick("lead"), "actions": pick("actions"), "effect": pick("effect"), "reasons": pick("reasons"), "risks": pick("risks"), "steps": pick("steps")})
}

/// What is known about the proposal's place: the report, client, period, who.
pub struct Context<'a> {
    pub client: &'a str,
    pub client_id: &'a str,
    pub report: &'a str,
    pub period: &'a str,
    pub file: Option<&'a str>,
    pub user: &'a str,
    pub user_name: &'a str,
}

/// Records a proposal made or changed: a new entry, or a new version of `uid`'s.
/// `source`: "notes" (laid out as written), "ai" (written up by the AI from notes), "draft" (the AI's
/// proposal ideas), with `ai_at` the AI request's time in the AI log when there was one.
#[allow(clippy::too_many_arguments)]
pub fn record(dir: &Path, uid: &str, spec: &Json, notes: &str, source: &str, ai_at: Option<&str>, cx: &Context, pages: usize) -> Result<Json, String> {
    let mut all = load(dir);
    let at = now();
    let body = content(spec);
    let place = json!({"client": cx.client, "client_id": cx.client_id, "report": cx.report, "period": cx.period, "file": cx.file, "pages": pages});
    let entry = match all.iter_mut().find(|r| r["uid"] == uid) {
        Some(r) => {
            // the content it had becomes a version (when it changed)
            if r["content"] != body || r["notes"] != json!(notes) {
                let v = json!({"at": r["updated"], "by": r["updated_by"], "content": r["content"], "notes": r["notes"], "source": r["source"]});
                let vs = r["versions"].as_array_mut();
                match vs {
                    Some(vs) => { vs.push(v); if vs.len() > MAX_VERSIONS { vs.remove(0); } }
                    None => r["versions"] = json!([v]),
                }
            }
            r["content"] = body;
            r["notes"] = json!(notes);
            r["source"] = json!(source);
            if let Some(a) = ai_at { r["ai_at"] = json!(a); }
            r["place"] = place;
            r["updated"] = json!(at);
            r["updated_by"] = json!(cx.user);
            r["in_report"] = json!(true);
            r.clone()
        }
        None => {
            let r = json!({"uid": uid, "created": at, "created_by": cx.user, "banker": cx.user_name, "updated": at, "updated_by": cx.user,
                "content": body, "notes": notes, "source": source, "ai_at": ai_at, "place": place, "in_report": true,
                "status": "draft", "history": [{"at": at, "by": cx.user, "status": "draft", "note": ""}], "versions": []});
            all.push(r.clone());
            r
        }
    };
    save(dir, &all)?;
    Ok(entry)
}

/// The proposal left the report (it stays in the register, marked as no longer in a report).
pub fn removed(dir: &Path, uid: &str, user: &str) -> Result<(), String> {
    let mut all = load(dir);
    if let Some(r) = all.iter_mut().find(|r| r["uid"] == uid) {
        r["in_report"] = json!(false);
        r["updated"] = json!(now());
        r["updated_by"] = json!(user);
        save(dir, &all)?;
    }
    Ok(())
}

/// A new status (with an optional note), kept in the proposal's history.
pub fn set_status(dir: &Path, uid: &str, status: &str, note: &str, user: &str) -> Result<Json, String> {
    if !STATUSES.contains(&status) {
        return Err(format!("“{status}” isn't a proposal status."));
    }
    let mut all = load(dir);
    let r = all.iter_mut().find(|r| r["uid"] == uid).ok_or("That proposal is no longer in the register.")?;
    let at = now();
    r["status"] = json!(status);
    r["status_at"] = json!(at);
    match r["history"].as_array_mut() {
        Some(h) => h.push(json!({"at": at, "by": user, "status": status, "note": note})),
        None => r["history"] = json!([{"at": at, "by": user, "status": status, "note": note}]),
    }
    r["updated"] = json!(at);
    r["updated_by"] = json!(user);
    let out = r.clone();
    save(dir, &all)?;
    Ok(out)
}

/// A report with the proposal in it went out (exported): a draft becomes presented.
pub fn exported(dir: &Path, uids: &[String], user: &str, how: &str) -> Result<(), String> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut all = load(dir);
    let at = now();
    let mut changed = false;
    for r in all.iter_mut().filter(|r| r["uid"].as_str().is_some_and(|u| uids.iter().any(|x| x == u))) {
        r["exported"] = json!(at);
        if r["status"] == "draft" {
            r["status"] = json!("presented");
            r["status_at"] = json!(at);
            if let Some(h) = r["history"].as_array_mut() {
                h.push(json!({"at": at, "by": user, "status": "presented", "note": how}));
            }
        }
        changed = true;
    }
    if changed { save(dir, &all)?; }
    Ok(())
}

/// The register's entries, newest first: all of them (`clients` = None), or those for the clients
/// named; `client` narrows to one.
pub fn list(dir: Option<&Path>, clients: Option<&[String]>, client: Option<&str>) -> Vec<Json> {
    let Some(d) = dir else { return Vec::new() };
    let mut all: Vec<Json> = load(d).into_iter().filter(|r| {
        let c = r["place"]["client"].as_str().unwrap_or("");
        clients.is_none_or(|cs| cs.iter().any(|x| x == c)) && client.is_none_or(|x| x == c)
    }).collect();
    all.sort_by(|a, b| b["updated"].as_str().unwrap_or("").cmp(a["updated"].as_str().unwrap_or("")));
    all
}

/// One entry.
pub fn get(dir: Option<&Path>, uid: &str) -> Option<Json> {
    load(dir?).into_iter().find(|r| r["uid"] == uid)
}

/// Presented proposals with no outcome for `FOLLOW_UP_DAYS`: a reminder each.
pub fn follow_ups(entries: &[Json], today: chrono::NaiveDate) -> Vec<Json> {
    entries.iter().filter(|r| r["status"] == "presented").filter_map(|r| {
        let since = r["status_at"].as_str().or(r["exported"].as_str()).or(r["updated"].as_str())?;
        let d = chrono::DateTime::parse_from_rfc3339(since).ok()?.date_naive();
        let days = (today - d).num_days();
        (days >= FOLLOW_UP_DAYS).then(|| json!({"uid": r["uid"], "title": r["content"]["title"], "client": r["place"]["client"], "since": d.to_string(), "days": days}))
    }).collect()
}

/// The earlier proposals for a client as the report's AI reads them: when, what, status, actions.
pub fn for_ai(entries: &[Json]) -> String {
    let mut out = String::new();
    for r in entries.iter().take(8) {
        let acts: Vec<String> = r["content"]["actions"].as_array().into_iter().flatten().take(6)
            .map(|a| [a["action"].as_str(), a["what"].as_str(), a["amount"].as_str()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")).collect();
        let last_note = r["history"].as_array().and_then(|h| h.last()).and_then(|h| h["note"].as_str()).filter(|n| !n.is_empty()).map(|n| format!(" — note: {n}")).unwrap_or_default();
        out.push_str(&format!("- {} «{}» · status: {}{} · actions: {}\n",
            r["created"].as_str().unwrap_or("").get(..10).unwrap_or(""), r["content"]["title"].as_str().unwrap_or(""), r["status"].as_str().unwrap_or(""), last_note, acts.join("; ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proposal_is_kept_with_versions_status_and_follow_ups() {
        let dir = std::env::temp_dir().join(format!("quire_proposals_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cx = Context { client: "James Hardenberg", client_id: "C0017", report: "Отчёт", period: "Q3 2026", file: None, user: "user", user_name: "Warren Buffington" };
        let uid = new_uid();
        let spec = json!({"title": "План", "actions": [{"action": "Купить", "what": "ОФЗ", "amount": "5 млн ₽"}]});
        let r = record(&dir, &uid, &spec, "План\n- Купить ОФЗ", "notes", None, &cx, 1).unwrap();
        assert_eq!((r["status"].as_str(), r["place"]["client"].as_str()), (Some("draft"), Some("James Hardenberg")));
        // changed: the old content is a version
        let r = record(&dir, &uid, &json!({"title": "План Б"}), "План Б", "ai", Some("2026-10-23T10:00:00"), &cx, 2).unwrap();
        assert_eq!(r["versions"].as_array().unwrap().len(), 1);
        assert_eq!(r["versions"][0]["content"]["title"], "План");
        assert_eq!(r["ai_at"], "2026-10-23T10:00:00");
        // exported: presented; then an outcome with a note
        exported(&dir, std::slice::from_ref(&uid), "user", "PDF").unwrap();
        assert_eq!(get(Some(&dir), &uid).unwrap()["status"], "presented");
        let today = chrono::Local::now().date_naive();
        assert!(follow_ups(&list(Some(&dir), None, None), today).is_empty(), "not yet");
        assert_eq!(follow_ups(&list(Some(&dir), None, None), today + chrono::Duration::days(15)).len(), 1, "two weeks on");
        let r = set_status(&dir, &uid, "accepted", "Согласен на ОФЗ", "user").unwrap();
        assert_eq!(r["history"].as_array().unwrap().len(), 3);
        assert!(set_status(&dir, &uid, "maybe", "", "user").is_err());
        // only the clients asked for
        assert_eq!(list(Some(&dir), Some(&["Someone else".to_string()]), None).len(), 0);
        assert_eq!(list(Some(&dir), Some(&["James Hardenberg".to_string()]), None).len(), 1);
        assert!(for_ai(&list(Some(&dir), None, None)).contains("«План Б» · status: accepted — note: Согласен на ОФЗ"));
        // out of the report: still in the register
        removed(&dir, &uid, "user").unwrap();
        assert_eq!(get(Some(&dir), &uid).unwrap()["in_report"], false);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
