//! The banker's client list, with something to read: each client's value,
//! return and flows this year, risk profile and products, and what needs
//! attention soon (payments due, policies ending, birthdays, large
//! withdrawals). All from the bank's own tables, as of today (the last day
//! with data on or before it); returns follow the report's rule
//! (net_twr_ltd at the end / the day before the start − 1).

use crate::auth::{query, quote_list};
use serde_json::{json, Value as Json};

/// Days ahead that count as "coming up".
const PAYMENTS_DAYS: i64 = 30;
const POLICY_DAYS: i64 = 90;
const BIRTHDAY_DAYS: i64 = 14;
/// A withdrawal in the last 30 days worth flagging: this share of the portfolio, or this much.
const WITHDRAW_SHARE: f64 = 0.05;
const WITHDRAW_RUB: f64 = 10_000_000.0;
/// Cash worth a word: this share of the portfolio and at least this much.
const CASH_SHARE: f64 = 0.10;
const CASH_RUB: f64 = 5_000_000.0;
/// The most a risk profile holds in shares before the client is flagged (an assumption: the bank's own limits go here).
fn equity_limit(profile: &str) -> Option<f64> {
    match profile {
        "Консервативный" => Some(0.40),
        "Умеренный" => Some(0.60),
        "Умеренно-агрессивный" => Some(0.80),
        _ => None,
    }
}

/// An account number as the report shows it: its last five characters.
fn short_no(n: &str) -> String {
    let c: Vec<char> = n.chars().collect();
    if c.len() <= 5 { n.to_string() } else { format!("***{}", c[c.len() - 5..].iter().collect::<String>()) }
}

fn num(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

/// The overview, kept for the day: the same clients, products and database file (unchanged
/// since) are answered from memory instead of being worked out again.
pub fn overview_cached(path: &str, ids: &[String], sources: &str, today: chrono::NaiveDate) -> Result<Json, String> {
    use std::hash::{Hash, Hasher};
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Json>>> = std::sync::OnceLock::new();
    let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ids.hash(&mut h);
    let key = format!("{path}|{stamp}|{today}|{sources}|{:x}", h.finish());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(v);
    }
    let v = overview(path, ids, sources, today)?;
    if let Ok(mut c) = cache.lock() {
        // a handful of people and days at most
        if c.len() > 16 {
            c.clear();
        }
        c.insert(key, v.clone());
    }
    Ok(v)
}

/// `ids`: the clients to describe; `sources`: the products the person may see ("" = all).
pub fn overview(path: &str, ids: &[String], sources: &str, today: chrono::NaiveDate) -> Result<Json, String> {
    if ids.is_empty() {
        return Ok(json!({ "as_of": Json::Null, "clients": {}, "attention": [] }));
    }
    let t = format!("date '{}'", today.format("%Y-%m-%d"));
    let base = format!(
        "with ids as (select unnest([{ids}]) as id),
        d as (select max(report_dt) as d, (date_trunc('year', max(report_dt)) - interval 1 day)::date as y0 from portf_acct where report_dt <= {t}),
        acc as (select a.* from accounts a where a.client_id in (select id from ids)
          and ('{src}' = '' or list_contains(string_split('{src}', ','), a.src_cd))),",
        ids = quote_list(ids),
        src = sources.replace('\'', "''"),
    );
    // value, start of the year, return and net flow this year, a value per month-end
    let figs = query(path, &format!("{base}
        pa as (select p.client_id, p.report_dt, sum(bal_in) bi, sum(net_flow) nf, sum(bal_out) bo, sum(fees) fe
               from portf_acct p join acc using (acct_cd) where p.report_dt <= (select d from d) group by 1, 2),
        lt as (select *, exp(sum(ln(case when tw > 0 then tw else 1 end)) over (partition by client_id order by report_dt)) ltd
               from (select *, case when nf >= 0 then bo / nullif(bi + nf, 0) else (bo - nf) / nullif(bi, 0) end tw from pa))
        select client_id, (select d from d)::varchar,
          max(bo) filter (report_dt = (select d from d))::varchar,
          coalesce(max(bo) filter (report_dt = (select y0 from d)), 0)::varchar,
          (max(ltd) filter (report_dt = (select d from d)) / coalesce(max(ltd) filter (report_dt = (select y0 from d)), 1) - 1)::varchar,
          coalesce(sum(nf) filter (report_dt > (select y0 from d)), 0)::varchar,
          string_agg(round(bo)::bigint::varchar, ',' order by report_dt)
            filter (report_dt > (select d from d) - interval 12 month and (report_dt = last_day(report_dt) or report_dt = (select d from d))),
          coalesce(sum(fe) filter (report_dt > (select y0 from d)), 0)::varchar
        from lt group by 1"), 8)?;
    // what the portfolio holds now: cash and shares
    let mix = query(path, &format!("{base} x as (select 1)
        select b.client_id, coalesce(sum(b.amt_rub) filter (i.instr_type = 'Currency'), 0)::varchar,
          coalesce(sum(b.amt_rub) filter (i.instr_type = 'Equity'), 0)::varchar, sum(b.amt_rub)::varchar
        from balances b join acc using (acct_cd) join instruments i using (instr_cd)
        where b.report_dt = (select d from d) group by 1"), 4)?;
    // every client's return this year with their risk profile, for "vs peers" (as the report: the median of the others)
    let peers = query(path, &format!("with d as (select max(report_dt) as d, (date_trunc('year', max(report_dt)) - interval 1 day)::date as y0 from portf_acct where report_dt <= {t}),
        pd as (select p.client_id, p.report_dt, sum(bal_in) bi, sum(net_flow) nf, sum(bal_out) bo from portf_acct p
               where p.report_dt > (select y0 from d) and p.report_dt <= (select d from d) group by 1, 2),
        tw as (select *, case when nf >= 0 then bo / nullif(bi + nf, 0) else (bo - nf) / nullif(bi, 0) end tw from pd)
        select c.id, coalesce(c.risk_profile, ''), (exp(sum(ln(case when tw > 0 then tw else 1 end))) - 1)::varchar
        from tw join clients c on c.id = tw.client_id group by 1, 2"), 3).unwrap_or_default();
    let prods = query(path, &format!("{base} x as (select 1)
        select a.client_id, string_agg(distinct a.src_cd, ',' order by a.src_cd) from acc a, d
        where a.close_dt is null or a.close_dt > d.d group by 1"), 2)?;
    let people = query(path, &format!("{base} x as (select 1)
        select id, name, coalesce(risk_profile, ''), coalesce(birthday::varchar, '') from clients where id in (select id from ids)"), 4)?;
    // per account: which holding pays, how much a unit and in what currency
    let pays = query(path, &format!("{base}
        pos as (select b.client_id, b.acct_cd, b.instr_cd, sum(b.qty) qty from balances b join acc using (acct_cd)
                where b.report_dt = (select d from d) group by 1, 2, 3 having sum(b.qty) <> 0)
        select p.client_id, cs.pay_dt::varchar, cs.cf_type, i.instr_name, (p.qty * cs.amount_per_unit * coalesce(fx.rate, 1))::varchar,
          coalesce(i.instr_type, ''), coalesce(i.isin, ''), coalesce(i.coupon::varchar, ''), coalesce(i.maturity_dt::varchar, ''),
          p.qty::varchar, cs.amount_per_unit::varchar, cs.cur, a.acct_name, a.acct_numb
        from cashflow_schedule cs join pos p using (instr_cd) join instruments i using (instr_cd) join acc a on a.acct_cd = p.acct_cd
        left join fx_rates fx on fx.cur = cs.cur and fx.report_dt = (select d from d)
        where cs.pay_dt > {t} and cs.pay_dt <= {t} + {PAYMENTS_DAYS} and cs.cf_type in ('Купон', 'Дивиденд', 'Погашение')
        order by 2, 1"), 14)?;
    let policies = query(path, &format!("{base} x as (select 1)
        select client_id, coalesce(acct_type, ''), coalesce(acct_name, ''), policy_end_dt::varchar, acct_numb, coalesce(open_dt::varchar, '') from acc
        where policy_end_dt > {t} and policy_end_dt <= {t} + {POLICY_DAYS} order by 4"), 6)?;
    // each withdrawal in the last 30 days (they are summed per client below)
    let outs = query(path, &format!("{base} x as (select 1)
        select o.client_id, abs(o.amount_rub)::varchar, o.op_dt::varchar, a.acct_name, a.acct_numb from operations o join acc a using (acct_cd)
        where o.op_type = 'Вывод' and o.op_dt > {t} - 30 and o.op_dt <= {t} order by 3"), 5)?;

    // the products' names, from the bank's product table when it has one
    let names = query(path, "select src_cd, coalesce(name, ''), coalesce(name_en, '') from products order by sort_order", 3).unwrap_or_default();
    let mut clients = serde_json::Map::new();
    let mut as_of = Json::Null;
    for r in &figs {
        as_of = json!(r[1]);
        let spark: Vec<f64> = r[6].split(',').filter(|s| !s.is_empty()).map(num).collect();
        clients.insert(r[0].clone(), json!({ "value": num(&r[2]), "start": num(&r[3]), "ret": r[4].parse::<f64>().ok(), "flow": num(&r[5]), "spark": spark, "fees": -num(&r[7]) }));
    }
    let entry = |m: &mut serde_json::Map<String, Json>, id: &str| -> () {
        m.entry(id.to_string()).or_insert_with(|| json!({ "value": Json::Null, "spark": [] }));
    };
    for r in &prods {
        entry(&mut clients, &r[0]);
        clients[&r[0]]["products"] = json!(r[1].split(',').collect::<Vec<_>>());
    }
    let mut attention: Vec<Json> = Vec::new();
    let name_of = |id: &str| people.iter().find(|p| p[0] == id).map(|p| p[1].clone()).unwrap_or_default();
    for p in &people {
        entry(&mut clients, &p[0]);
        clients[&p[0]]["risk"] = json!(p[2]);
        // the next birthday, if within the window
        if let Ok(b) = chrono::NaiveDate::parse_from_str(&p[3], "%Y-%m-%d") {
            use chrono::Datelike;
            let on = |y: i32| chrono::NaiveDate::from_ymd_opt(y, b.month(), b.day()).or_else(|| chrono::NaiveDate::from_ymd_opt(y, 3, 1));
            let next = on(today.year()).filter(|d| *d >= today).or_else(|| on(today.year() + 1));
            if let Some(n) = next {
                let days = (n - today).num_days();
                if days <= BIRTHDAY_DAYS {
                    attention.push(json!({ "kind": "birthday", "client": p[1], "id": p[0], "date": n.to_string(), "age": n.year() - b.year() }));
                }
            }
        }
    }
    // return against clients of the same risk profile
    for (id, c) in clients.iter_mut() {
        let Some(me) = c["ret"].as_f64() else { continue };
        let Some(profile) = peers.iter().find(|p| &p[0] == id).map(|p| p[1].clone()) else { continue };
        let mut others: Vec<f64> = peers.iter().filter(|p| p[1] == profile && &p[0] != id).filter_map(|p| p[2].parse().ok()).collect();
        if others.is_empty() {
            continue;
        }
        others.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = others.len();
        let median = if n % 2 == 1 { others[n / 2] } else { (others[n / 2 - 1] + others[n / 2]) / 2.0 };
        c["vs_peers"] = json!(me - median);
        c["peers"] = json!(n);
    }
    // cash sitting idle, and more in shares than the risk profile allows
    for r in &mix {
        let (cash, eq, total) = (num(&r[1]), num(&r[2]), num(&r[3]));
        if total <= 0.0 {
            continue;
        }
        entry(&mut clients, &r[0]);
        clients[&r[0]]["cash"] = json!(cash / total);
        clients[&r[0]]["equity"] = json!(eq / total);
        if cash / total >= CASH_SHARE && cash >= CASH_RUB {
            attention.push(json!({ "kind": "cash", "client": name_of(&r[0]), "id": r[0], "date": as_of.clone(), "amount": cash, "share": cash / total, "value": total }));
        }
        let profile = clients[&r[0]]["risk"].as_str().unwrap_or("").to_string();
        if let Some(limit) = equity_limit(&profile) {
            if eq / total > limit {
                attention.push(json!({ "kind": "risk", "client": name_of(&r[0]), "id": r[0], "date": as_of.clone(), "amount": eq, "share": eq / total,
                    "limit": limit, "profile": profile, "value": total }));
            }
        }
    }
    // one item per payment: the same holding in several accounts pays once, from those accounts
    let mut merged: Vec<Json> = Vec::new();
    for r in &pays {
        let acct = format!("{} {}", r[12], short_no(&r[13]));
        if let Some(m) = merged.iter_mut().find(|m| m["id"] == r[0] && m["date"] == r[1] && m["type"] == r[2] && m["what"] == r[3]) {
            m["amount"] = json!(m["amount"].as_f64().unwrap_or(0.0) + num(&r[4]));
            m["qty"] = json!(m["qty"].as_f64().unwrap_or(0.0) + num(&r[9]));
            m["accounts"].as_array_mut().unwrap().push(json!(acct));
            continue;
        }
        merged.push(json!({ "kind": "payment", "client": name_of(&r[0]), "id": r[0], "date": r[1], "type": r[2], "what": r[3], "amount": num(&r[4]),
            "instr_type": r[5], "isin": r[6], "coupon": r[7].parse::<f64>().ok(), "maturity": r[8], "qty": num(&r[9]), "per_unit": num(&r[10]), "cur": r[11],
            "accounts": [acct] }));
    }
    attention.extend(merged);
    for r in &policies {
        attention.push(json!({ "kind": "policy", "client": name_of(&r[0]), "id": r[0], "date": r[3], "type": r[1], "what": r[2], "acct_no": short_no(&r[4]), "opened": r[5] }));
    }
    let mut out_by: std::collections::BTreeMap<String, Vec<&Vec<String>>> = Default::default();
    for r in &outs {
        out_by.entry(r[0].clone()).or_default().push(r);
    }
    for (id, ops) in out_by {
        let amount: f64 = ops.iter().map(|r| num(&r[1])).sum();
        let value = clients.get(&id).and_then(|c| c["value"].as_f64()).unwrap_or(0.0);
        let share = if value + amount > 0.0 { amount / (value + amount) } else { 0.0 };
        if share >= WITHDRAW_SHARE || amount >= WITHDRAW_RUB {
            let list: Vec<Json> = ops.iter().map(|r| json!({ "date": r[2], "amount": num(&r[1]), "account": r[3], "acct_no": short_no(&r[4]) })).collect();
            attention.push(json!({ "kind": "withdrawal", "client": name_of(&id), "id": id, "date": ops.last().map(|r| r[2].clone()).unwrap_or_default(),
                "amount": amount, "share": share, "value": value, "ops": list }));
        }
    }
    Ok(json!({ "as_of": as_of, "today": today.to_string(), "clients": clients, "attention": attention,
        "products": names.iter().map(|r| json!({ "id": r[0], "name": r[1], "name_en": r[2] })).collect::<Vec<_>>() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_reads_figures_and_whats_coming() {
        let Some(path) = crate::dbconn::demo_path() else { return };
        let ids: Vec<String> = ["C0017", "C1337"].iter().map(|s| s.to_string()).collect();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let o = overview(&path, &ids, "", today).unwrap();
        assert_eq!(o["as_of"], "2026-10-22");
        let c = &o["clients"]["C0017"];
        assert!(c["value"].as_f64().unwrap() > 1e8);
        assert!(c["ret"].as_f64().unwrap().abs() < 1.0);
        assert!(c["spark"].as_array().unwrap().len() >= 12);
        assert!(c["products"].as_array().unwrap().iter().any(|p| p == "BROK"));
        assert_eq!(c["risk"].as_str().map(|s| !s.is_empty()), Some(true));
        let att = o["attention"].as_array().unwrap();
        assert!(att.iter().any(|a| a["kind"] == "payment" && a["id"] == "C0017"), "the deposit maturing in November");
        assert!(att.iter().any(|a| a["kind"] == "withdrawal" && a["id"] == "C1337"), "a large withdrawal is flagged");
        assert!(att.iter().all(|a| a["date"].as_str().unwrap_or("") >= "2026-09-22"));
        // fees, return against peers, and the holdings' flags
        assert!(c["fees"].as_f64().unwrap() > 0.0);
        assert!(c["vs_peers"].as_f64().is_some() && c["peers"].as_u64().unwrap() > 3);
        let all: Vec<String> = ["C0001", "C0009", "C0017", "C0025", "C0033", "C1337"].iter().map(|s| s.to_string()).collect();
        let o6 = overview(&path, &all, "", today).unwrap();
        let flagged = |k: &str| { let mut v: Vec<String> = o6["attention"].as_array().unwrap().iter().filter(|a| a["kind"] == k).map(|a| a["id"].as_str().unwrap().to_string()).collect(); v.sort(); v };
        assert_eq!(flagged("cash"), ["C0025", "C0033"], "10% or more in cash");
        assert_eq!(flagged("risk"), ["C0001", "C0009", "C0033"], "conservative clients over 40% in shares");
        // kept for the day: the second time comes from memory, the same answer
        let a = overview_cached(&path, &ids, "", today).unwrap();
        let t = std::time::Instant::now();
        assert_eq!(overview_cached(&path, &ids, "", today).unwrap(), a);
        assert!(t.elapsed().as_millis() < 20, "from memory");
        // only the products the person may see
        let bank_only = overview(&path, &ids, "BANK", today).unwrap();
        assert_eq!(bank_only["clients"]["C0017"]["products"], json!(["BANK"]));
        assert!(bank_only["clients"]["C0017"]["value"].as_f64().unwrap() < c["value"].as_f64().unwrap());
    }
}
