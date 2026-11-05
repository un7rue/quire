//! The report user's AI assistant: a brief of the report with what to pay attention to and how
//! to present it to the client, a check before sending, and a draft note for a page.
//!
//! Unlike the data and design assistants it needs the client's figures, so it runs only when the
//! bank allows aggregates to leave (AI policy level "aggregates" or "full"). It sends the
//! figures the report itself shows — page by page, widget by widget, at most a dozen rows each —
//! never raw data, and with identities masked: the client, account and contract numbers, ISINs
//! and other codes become placeholders ([CLIENT], [ID-3]) that are put back in the answer here.

use crate::model::Doc;
use serde_json::Value as Json;

/// Swaps identities for placeholders on the way out and back on the way in.
#[derive(Default, Debug, Clone)]
pub struct Masker {
    pairs: Vec<(String, String)>,
}

impl Masker {
    /// Masks the client's name (and any other exact strings given) wherever it appears.
    pub fn new(exact: &[String]) -> Masker {
        let mut m = Masker::default();
        let mut ex: Vec<&String> = exact.iter().filter(|s| s.chars().count() >= 3).collect();
        ex.sort_by_key(|s| std::cmp::Reverse(s.len()));
        for (i, s) in ex.into_iter().enumerate() {
            let tok = if i == 0 { "[CLIENT]".to_string() } else { format!("[NAME-{i}]") };
            m.pairs.push((s.clone(), tok));
        }
        m
    }

    /// A name the report shows as a category — an instrument, an account, a person — masked
    /// wherever it appears: answers its placeholder ([INS-2], [ACC-1], [NAME-1]).
    pub fn name(&mut self, value: &str, kind: &str) -> String {
        let v = value.trim();
        if v.chars().count() < 2 || v.starts_with('[') {
            return value.to_string();
        }
        if let Some((_, t)) = self.pairs.iter().find(|(o, _)| o == v) {
            return t.clone();
        }
        let n = self.pairs.iter().filter(|(_, t)| t.starts_with(&format!("[{kind}-"))).count() + 1;
        let t = format!("[{kind}-{n}]");
        self.pairs.push((v.to_string(), t.clone()));
        t
    }

    /// Another spelling of something already masked (a person's login beside their name): the same placeholder.
    pub fn alias(&mut self, value: &str, token: &str) {
        let v = value.trim();
        if v.chars().count() >= 2 && !self.pairs.iter().any(|(o, _)| o == v) {
            self.pairs.push((v.to_string(), token.to_string()));
        }
    }

    fn token_for(&mut self, word: &str) -> String {
        if let Some((_, t)) = self.pairs.iter().find(|(o, _)| o == word) {
            return t.clone();
        }
        let t = format!("[ID-{}]", self.pairs.iter().filter(|(_, t)| t.starts_with("[ID-")).count() + 1);
        self.pairs.push((word.to_string(), t.clone()));
        t
    }

    /// An identifier: an account or contract number, an ISIN, a code — letters and digits
    /// together, or digits behind *** or №. Amounts, percentages and dates are not.
    /// Whether a text holds an identifier (an account number like ***00513, a code): then only
    /// the identifier is masked and the words around it — a product's name — stay.
    pub fn has_id(text: &str) -> bool {
        text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '«' | '»' | '·')).any(|w| Masker::is_id(w.trim_end_matches(['.', ',', ':'])))
    }

    fn is_id(word: &str) -> bool {
        let digits = word.chars().filter(|c| c.is_ascii_digit()).count();
        let letters = word.chars().filter(|c| c.is_alphabetic()).count();
        (word.starts_with("***") && digits >= 2) || (word.starts_with('№') && digits >= 2) || (digits >= 2 && letters >= 1 && !word.ends_with('%'))
    }

    pub fn mask(&mut self, text: &str) -> String {
        let mut s = text.to_string();
        // names first, longest first, as whole words only (a short name never cuts into another word)
        let mut names: Vec<(String, String)> = self.pairs.iter().filter(|(_, t)| !t.starts_with("[ID-")).cloned().collect();
        names.sort_by_key(|(o, _)| std::cmp::Reverse(o.chars().count()));
        for (o, t) in &names {
            s = replace_words(&s, o, t);
        }
        // word by word: separators stay as they are
        let mut out = String::with_capacity(s.len());
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut String, m: &mut Masker| {
            if !word.is_empty() {
                let w = std::mem::take(word);
                let trimmed = w.trim_end_matches(['.', ',', ':']);
                if !trimmed.starts_with('[') && Masker::is_id(trimmed) {
                    out.push_str(&m.token_for(trimmed));
                    out.push_str(&w[trimmed.len()..]);
                } else {
                    out.push_str(&w);
                }
            }
        };
        for c in s.chars() {
            if c.is_whitespace() || matches!(c, '(' | ')' | '«' | '»' | '"' | ';' | '|' | '·') {
                flush(&mut word, &mut out, self);
                out.push(c);
            } else {
                word.push(c);
            }
        }
        flush(&mut word, &mut out, self);
        out
    }

    pub fn unmask(&self, text: &str) -> String {
        let mut s = text.to_string();
        // longest tokens first, so [ID-12] isn't read as [ID-1] + "2]"
        let mut pairs: Vec<&(String, String)> = self.pairs.iter().collect();
        pairs.sort_by_key(|(_, t)| std::cmp::Reverse(t.len()));
        for (o, t) in pairs {
            s = s.replace(t.as_str(), o);
        }
        s
    }

    /// Every string in a JSON answer, put back.
    pub fn unmask_json(&self, v: &mut Json) {
        match v {
            Json::String(s) => *s = self.unmask(s),
            Json::Array(a) => a.iter_mut().for_each(|x| self.unmask_json(x)),
            Json::Object(o) => o.values_mut().for_each(|x| self.unmask_json(x)),
            _ => {}
        }
    }
}

/// `from` replaced by `to` where it stands as a whole word (not inside a longer word).
fn replace_words(s: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return s.to_string();
    }
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(from) {
        let before = rest[..i].chars().next_back().or_else(|| out.chars().next_back());
        let after = rest[i + from.len()..].chars().next();
        out.push_str(&rest[..i]);
        if word(before) || word(after) {
            out.push_str(from);
        } else {
            out.push_str(to);
        }
        rest = &rest[i + from.len()..];
    }
    out.push_str(rest);
    out
}

/// Whether a source lists positions or instruments: it has their fields (ISIN, quantity,
/// price…) or says so in its name. Its "name" column then names instruments (masked);
/// elsewhere a name is a category, a benchmark or a product (kept).
fn lists_instruments(ds: &crate::model::Dataset) -> bool {
    let fields = ds.fields.iter().any(|f| {
        let k = f.name.to_lowercase();
        ["isin", "instr", "ticker", "qty", "quantity", "price", "avg", "coupon", "maturity", "nominal", "lots"].iter().any(|p| k.starts_with(p))
    });
    let n = ds.name.to_lowercase();
    fields || ["позиц", "инструм", "бумаг", "position", "holding", "instrument", "securit"].iter().any(|w| n.contains(w))
}

/// What a category column holds, when it names a person or an instrument (masked). Account
/// numbers and other codes are masked wherever they appear, so accounts need no column rule.
fn identity_kind(key: &str, instruments: bool) -> Option<&'static str> {
    let k = key.to_lowercase();
    let k = k.trim_start_matches('_');
    if ["client", "holder", "owner", "person", "manager", "banker", "sales"].iter().any(|p| k.starts_with(p)) {
        Some("NAME")
    } else if ["instr", "isin", "ticker", "security", "issuer"].iter().any(|p| k.starts_with(p)) || (instruments && k == "name") {
        Some("INS")
    } else {
        None
    }
}

/// A figure as the report shows it, near enough: percentages with one decimal, amounts whole.
fn fmt(v: &Json, format: &str) -> String {
    let Some(x) = v.as_f64() else {
        return v.as_str().map(String::from).unwrap_or_default();
    };
    if format == "percent" {
        return format!("{:.1}%", x * 100.0);
    }
    // small figures (ratios, differences) keep their decimals
    if x.abs() < 10.0 && x.fract() != 0.0 {
        return format!("{}{}", if x < 0.0 { "−" } else { "" }, format!("{:.3}", x.abs()).trim_end_matches('0').trim_end_matches('.'));
    }
    let neg = x < 0.0;
    let mut n = format!("{:.0}", x.abs());
    let mut out = String::new();
    while n.len() > 3 {
        let tail = n.split_off(n.len() - 3);
        out = format!(" {tail}{out}");
    }
    format!("{}{n}{out}", if neg { "−" } else { "" })
}

/// A figure as a widget shows it: its own decimals, prefix and suffix (" млн ₽", " п.п.").
fn as_shown(v: &Json, format: &str, o: &Json) -> String {
    let body = match (v.as_f64(), o["decimals"].as_u64()) {
        (Some(x), Some(d)) if format != "percent" && x.abs() < 1e5 => {
            let t = format!("{:.*}", d as usize, x.abs());
            format!("{}{t}", if x < 0.0 { "−" } else { "" })
        }
        _ => fmt(v, format),
    };
    if v.as_f64().is_none() || body.is_empty() {
        return body;
    }
    format!("{}{body}{}", o["prefix"].as_str().unwrap_or(""), o["suffix"].as_str().unwrap_or(""))
}

const MAX_ROWS: usize = 12;

/// The report as the model may read it: each shown page with its widgets' figures and notes.
/// `only`: one page (its id). Answers the text and how many characters of figures it holds.
pub fn facts(doc: &Doc, st: &Json, only: Option<&str>, m: &mut Masker) -> String {
    let mut out = String::new();
    for (pi, (p, pv)) in doc.pages.iter().zip(st["pages"].as_array().into_iter().flatten()).enumerate() {
        if pv["hidden"].as_bool() == Some(true) || only.is_some_and(|id| id != p.id) {
            continue;
        }
        let num = pv["number"].as_u64().unwrap_or(pi as u64 + 1);
        let title = pv["title"].as_str().unwrap_or(&p.title);
        let mut body = String::new();
        // in reading order: top to bottom, left to right (as the page is read, not as it was built)
        let mut ws: Vec<(&crate::model::Widget, &Json)> = p.widgets.iter().zip(pv["widgets"].as_array().into_iter().flatten()).collect();
        ws.sort_by_key(|(w, _)| (w.y, w.x));
        for (w, wv) in ws {
            if w.opts.get("mirror").is_some() {
                continue;
            }
            let name = wv["title"].as_str().filter(|t| !t.trim().is_empty()).map(String::from)
                .unwrap_or_else(|| w.opt_str("name").map(String::from).unwrap_or_default());
            let d = &wv["data"];
            let ds = w.binding.as_ref().and_then(|b| doc.dataset(&b.dataset));
            let instruments = ds.is_some_and(lists_instruments);
            // words the source itself writes ("Налоги, комиссии и курсовая разница", a section's name) are labels, not identities
            let own_words = ds.map(|d| match &d.source {
                crate::model::Source::Sqlite { query, .. } | crate::model::Source::Postgres { query, .. } | crate::model::Source::Clickhouse { query, .. } | crate::model::Source::Duckdb { query, .. } | crate::model::Source::Db { query, .. } => query.clone(),
                crate::model::Source::Query { sql } => sql.clone(),
                crate::model::Source::Python { code } => code.clone(),
                _ => String::new(),
            }).unwrap_or_default();
            match w.kind.as_str() {
                "comment" | "text" => {
                    // its placeholders filled as the page shows them (the page's account too)
                    let t = crate::vm::with_row(&p.row, || crate::vm::expand(w.opt_str("text").unwrap_or(""), doc));
                    let t = t.trim();
                    // (a text still holding placeholders is boilerplate: left out)
                    if !t.is_empty() && !t.contains("{{") {
                        // long legal texts go whole, up to a limit that says so when it cuts
                        let cut = t.chars().count() > 3000;
                        let t: String = t.chars().take(3000).collect::<String>() + if cut { " … (the text goes on)" } else { "" };
                        body.push_str(&format!("- {} [{}]: {}\n", if w.kind == "comment" { "Note to the client" } else { "Text" }, w.id, t.replace('\n', " ")));
                    }
                }
                "heading" => {
                    let t = w.opt_str("text").unwrap_or("").trim();
                    if !t.is_empty() && !t.contains("{{") {
                        body.push_str(&format!("- Heading: {}\n", t.lines().next().unwrap_or("")));
                    }
                }
                _ if d.is_object() && d.get("error").is_none() => {
                    let cols = d["columns"].as_array().cloned().unwrap_or_default();
                    let label = |c: &Json| c["label"].as_str().unwrap_or("").to_string();
                    let format = |c: &Json| c["format"].as_str().unwrap_or("number").to_string();
                    if let Some(k) = d.get("kpi").filter(|k| k.is_object()) {
                        let f0 = cols.first().map(format).unwrap_or_default();
                        // a KPI's look (decimals, prefix, suffix) sits in its chart options
                        // (its design view's settings too, with the card's own on top)
                        let ev = crate::model::effective(&doc.theme, w);
                        let mut look = ev.opts.as_object().cloned().unwrap_or_default();
                        look.extend(ev.opts["viz"].as_object().cloned().unwrap_or_default());
                        let look = Json::Object(look);
                        let vals = k["values"].as_array().cloned().unwrap_or_default();
                        // "All values on one line": "17 · 13,0 млн" as the card draws it (counts whole, the last with the card's decimals)
                        if look["valueJoin"].as_bool() == Some(true) && look["kpiGroup"].as_str() != Some("shares") {
                            let last = vals.iter().rposition(|v| v.is_number());
                            let mut parts: Vec<String> = Vec::new();
                            if look["kpiGroup"].as_str().unwrap_or("name") == "name" {
                                if let Some(n) = k["name"].as_str().filter(|n| !n.is_empty()) { parts.push(n.to_string()); }
                            }
                            for (i, v) in vals.iter().enumerate().filter(|(_, v)| v.is_number()) {
                                let whole = v.as_f64().is_some_and(|x| x.fract() == 0.0) && Some(i) != last;
                                let f = cols.get(i).map(format).unwrap_or_default();
                                parts.push(if whole || f == "percent" { fmt(v, &f) } else { as_shown(v, &f, &serde_json::json!({"decimals": look["decimals"]})) });
                            }
                            body.push_str(&format!("- KPI «{name}»: {}{}\n", parts.join(" · "), look["suffix"].as_str().unwrap_or("")));
                            continue;
                        }
                        let mut line = format!("- KPI «{name}»: {}", as_shown(&k["value"], &f0, &look));
                        if let Some(n) = look["note"].as_str().filter(|n| !n.trim().is_empty() && !n.contains('{')) {
                            line.push_str(&format!(" ({})", n.trim()));
                        }
                        if let Some(p) = k["pct"].as_f64() {
                            line.push_str(&format!(" ({:+.1}% vs the comparison period)", p * 100.0));
                        }
                        // a breakdown: the other measures under it
                        let extra: Vec<String> = cols.iter().zip(vals.iter()).skip(1).map(|(c, v)| format!("{} {}", label(c), fmt(v, &format(c)))).collect();
                        if !extra.is_empty() {
                            line.push_str(&format!("; {}", extra.join("; ")));
                        }
                        body.push_str(&line);
                        body.push('\n');
                    } else if let Some(all) = d["rows"].as_array().filter(|r| !r.is_empty()) {
                        let kind = crate::vm::kind_name(&w.kind);
                        // a long table's piece shows only its own rows (a continuation starts later)
                        let t = &wv["table"];
                        let (from, shown) = match (t["offset"].as_u64(), t["visible"].as_u64()) {
                            (Some(o), Some(v)) if w.kind == "table" => ((o as usize).min(all.len()), v as usize),
                            _ => (0, all.len()),
                        };
                        let rows = &all[from..(from + shown).min(all.len())];
                        if rows.is_empty() {
                            continue;
                        }
                        let part = if rows.len() < all.len() { format!(", rows {}–{} of {}", from + 1, from + rows.len(), all.len()) } else { String::new() };
                        // a table as it is drawn: its hidden columns left out, its own column labels
                        let style = |c: &Json| -> Json {
                            let ts = &w.opts["table"]["cols"];
                            let mut s = ts[c["base"].as_str().unwrap_or("")].as_object().cloned().unwrap_or_default();
                            s.extend(ts[c["key"].as_str().unwrap_or("")].as_object().cloned().unwrap_or_default());
                            Json::Object(s)
                        };
                        let is_table = w.kind == "table";
                        // a chart's numbers with its own decimals and unit (" п.п.", one decimal…)
                        let ev = crate::model::effective(&doc.theme, w);
                        let chart_look = serde_json::json!({"decimals": ev.opts["viz"]["decimals"], "suffix": ev.opts["viz"]["suffix"], "prefix": ev.opts["viz"]["prefix"]});
                        let shown_col: Vec<bool> = cols.iter().map(|c| !(is_table && style(c)["hidden"].as_bool() == Some(true))).collect();
                        let head = |c: &Json| style(c)["label"].as_str().filter(|_| is_table).map(String::from).unwrap_or_else(|| label(c)).replace('\n', " ");
                        let heads: Vec<String> = cols.iter().zip(&shown_col).filter(|x| *x.1).map(|(c, _)| head(c)).collect();
                        // its subtitle says what it measures ("доходность — результат к вложенному"): it goes with it
                        let sub = crate::vm::with_row(&p.row, || crate::vm::expand(&ev.subtitle, doc));
                        let sub = if sub.trim().is_empty() || sub.contains("{{") { String::new() } else { format!(" — {}", m.mask(sub.trim())) };
                        body.push_str(&format!("- {kind} «{name}»{sub} ({}{part}):\n", heads.join(" | ")));
                        for (ri, r) in rows.iter().enumerate().take(MAX_ROWS) {
                            // a group's row leaves out what only means something per row (rates, prices, quantities)
                            let group = is_table && d["row_meta"][from + ri]["kind"].as_str() == Some("group");
                            let blank = |c: &Json| group && c["role"].as_str() != Some("dim") && (w.opts["table"]["groupValues"].as_bool() == Some(false) || style(c)["groupHide"].as_bool() == Some(true));
                            // the names of instruments, accounts and people go out as placeholders
                            let cells: Vec<String> = r.as_array().into_iter().flatten().zip(cols.iter()).zip(&shown_col).filter(|x| *x.1).map(|(x, _)| x).map(|(v, c)| {
                                if blank(c) {
                                    return String::new();
                                }
                                let shown = if is_table { as_shown(v, &format(c), &style(c)) } else { as_shown(v, &format(c), &chart_look) };
                                match (c["role"].as_str(), identity_kind(c["key"].as_str().unwrap_or(""), instruments)) {
                                    // "Вклад «Максимальный» ***00513": the account number masked as everywhere else, the product's name kept
                                    (Some("dim"), Some(_)) if Masker::has_id(&shown) => m.mask(&shown),
                                    (Some("dim"), Some(kind)) if shown.chars().count() < 3 || !own_words.contains(shown.as_str()) => m.name(&shown, kind),
                                    _ => shown,
                                }
                            }).collect();
                            // a row with no figures is a section's heading
                            let figures = r.as_array().into_iter().flatten().zip(cols.iter()).zip(&shown_col).any(|((v, c), on)| *on && c["role"].as_str() != Some("dim") && !v.is_null() && v.as_str() != Some(""));
                            if figures {
                                body.push_str(&format!("    {}\n", cells.join(" | ")));
                            } else {
                                body.push_str(&format!("    — section: {}\n", cells.iter().filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>().join(" · ")));
                            }
                        }
                        if rows.len() > MAX_ROWS {
                            body.push_str(&format!("    … {} more rows\n", rows.len() - MAX_ROWS));
                        }
                        // a waterfall from a start to an end: the end bar it draws
                        if w.kind == "waterfall" && ev.opts["viz"]["bridge"].as_bool() == Some(true) {
                            let j = cols.iter().position(|c| c["role"].as_str() != Some("dim")).unwrap_or(1);
                            let end: f64 = all.iter().filter_map(|r| r[j].as_f64()).sum();
                            body.push_str(&format!("    {} | {}\n", ev.opts["viz"]["totalLabel"].as_str().unwrap_or("End"), as_shown(&serde_json::json!(end), &format(&cols[j]), &chart_look)));
                        }
                        // a total as the table shows it (charts add up nothing the reader sees)
                        // only when the table draws one: its own switch, and on its last piece
                        let totals = w.kind == "table" && t["totals"].as_bool().unwrap_or(w.opts.get("totals").and_then(|v| v.as_bool()) != Some(false)) && t["continued"].as_bool() != Some(true);
                        if let Some(tr) = d["total_row"].as_array().filter(|_| totals) {
                            // as the table draws it: each column's own total mode, numbers only
                            let cells: Vec<Option<String>> = cols.iter().enumerate().filter(|(j, _)| shown_col[*j]).map(|(j, c)| {
                                let nums: Vec<f64> = all.iter().filter_map(|r| r[j].as_f64()).collect();
                                let v = match style(c)["total"].as_str().unwrap_or("auto") {
                                    "none" => None,
                                    "sum" => (!nums.is_empty()).then(|| nums.iter().sum()),
                                    "avg" => (!nums.is_empty()).then(|| nums.iter().sum::<f64>() / nums.len() as f64),
                                    "min" => nums.iter().cloned().reduce(f64::min),
                                    "max" => nums.iter().cloned().reduce(f64::max),
                                    "count" => Some(nums.len() as f64),
                                    _ => tr.get(j).and_then(|v| v.as_f64()),
                                };
                                v.map(|x| as_shown(&serde_json::json!(x), &format(c), &style(c)))
                            }).collect();
                            if cells.iter().any(|x| x.is_some()) {
                                let cells: Vec<String> = cells.into_iter().enumerate().map(|(k, x)| x.unwrap_or_else(|| if k == 0 { "Total".into() } else { String::new() })).collect();
                                body.push_str(&format!("    {}\n", cells.join(" | ")));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if !body.is_empty() {
            // the banker's proposal: their recommendations, not the report's facts
            let what = if crate::proposal::is_proposal(p) { " — THE BANKER'S PROPOSAL" } else { "" };
            out.push_str(&format!("PAGE {num} «{title}» [{}]{what}\n{body}\n", p.id));
        }
    }
    m.mask(&out)
}

/// The system prompt for one of the tasks: "brief", "check" or "note".
pub fn system(kind: &str, report_lang: &str, ui_lang: &str) -> String {
    let rl = if report_lang.starts_with("ru") { "Russian" } else { "English" };
    let ul = if ui_lang == "ru" { "Russian" } else { "English" };
    let common = format!(
        "You assist a private banker who sends a client a portfolio report made in Quire. You see the figures the report shows, page by page.\n\
         Identities are masked: [CLIENT] is the client, [NAME-n] a person, [ACC-n] an account or contract, [INS-n] an instrument (a security, deposit or fund), [ID-n] a code —\n\
         use them as they are, never guess what they hide; the asset class or product shown beside them says what kind of thing they are.\n\
         Facts only from the report: never invent figures, events or market news; if something isn't in the report, say so. No investment advice or promises of returns.\n\
         A page marked THE BANKER'S PROPOSAL holds the banker's own recommendations to the client (actions, amounts, reasons): they are the banker's, not facts of the portfolio.\n\
         THE BANKER'S EARLIER PROPOSALS FOR THIS CLIENT (when listed) are past proposals with their status and notes: context, not figures of this report —\n\
         recall what was proposed and how it ended where it matters (e.g. a proposal still awaiting an answer, one the client accepted), never present them as new.\n\
         Refer to pages by their number, written in the language of the sentence (\"стр. 7\" in Russian, \"page 7\" in English); in JSON \"page\" fields give the number only."
    );
    let task = match kind {
        "check" => format!(
            "TASK: check the report before it is sent, like a careful second pair of eyes. Two kinds of finding:\n\
             ERRORS — something in the report is wrong and must be fixed: the same figure, measured the same way, shown differently on two pages; a label or note that says \
             something the figures don't (\"minus\" where the numbers add up); a wrong name, account or date; a total that doesn't add up.\n\
             SUGGESTIONS — worth a look, not wrong: a striking figure without a note (a big outflow, a loss, a concentration); wording a compliance officer would stop \
             (guaranteed or promised returns, a forecast stated as certain, advice).\n\
             BEFORE YOU CALL TWO FIGURES A CONTRADICTION, check they are the same thing. They are NOT a contradiction when they differ by rounding (figures shown rounded), \
             by scope (the whole portfolio vs one account vs a product grouping; regular income vs income with capital returned), by measure (TWR vs a simple return \
             on what was invested; before vs after tax; a price index vs a total-return index), or because a list shows only the largest items and says so. \
             Do the arithmetic before reporting it; if it adds up, don't report it.\n\
             You see figures, not the layout: don't judge positions, headings or how pages look. Don't report matters of taste or the client's own choices (their risk profile).\n\
             THE BANKER'S PROPOSAL, if there is one: don't judge whether it is a good idea; check that the figures it quotes agree with the report (a share, a value, an amount\n\
             larger than the money there is), that it doesn't contradict the report (selling what the client doesn't hold), and flag wording that promises returns.\n\
             Report every error you find; at most 6 suggestions, the most important first.\n\
             ANSWER with ONE JSON object: {{\"findings\": [{{\"page\": 7, \"type\": \"error|suggestion\", \"severity\": \"high|medium|low\", \"text\": \"what is wrong or worth a look, with both figures and pages, in {ul}\", \"fix\": \"what to do, in {ul}\"}}], \"verdict\": \"one sentence in {ul}\"}}\n\
             No errors → say the report is ready (suggestions may still be listed)."
        ),
        "proposal" => format!(
            "TASK: draft a proposal the banker could discuss with the client, from what this report shows. Look for: holdings outside the client's risk profile\n\
             (e.g. the share of shares against a conservative profile), a large share in one position or account, deposits or bonds coming back (maturities\n\
             in the forecast), idle cash, income to reinvest, weak results against the benchmarks or similar clients. 2–5 actions, the most important first.\n\
             GROUNDED: every action rests on figures the report shows (name the page in \"page\"); amounts only as the report gives them or simple parts of them\n\
             (\"часть из 18,8 млн ₽, которые вернутся с вклада 11.11\"); if the report already has THE BANKER'S PROPOSAL pages, suggest something\n\
             else — don't repeat their actions; don't suggest again what EARLIER PROPOSALS show as declined (build on accepted ones); instruments only those in the report, or a kind of product in general words\n\
             (\"облигации федерального займа\", \"вклад\", \"фонд денежного рынка\") — never invent a security, a rate, a price or a return. It is a draft\n\
             for the banker to decide on: ideas to discuss, not advice, no promises.\n\
             The page is written in {rl} to the client («Вы» in Russian), calm bank tone. It must fit ONE page — STRICT LENGTHS: title up to 60 characters;\n\
             lead up to 250 characters (1–2 sentences) with its main idea in **double asterisks**; at most 4 actions: \"action\" one short verb (Купить,\n\
             Продать, Сократить, Перевести, Открыть, Продлить, Вложить), \"what\" up to 40 characters, \"amount\" up to 25 characters (\"до 13,5 млн ₽\"),\n\
             \"why\" up to 50 characters, no markup in these; \"effect\", \"reasons\", \"risks\", \"steps\": at most 3 points each, up to 90 characters each,\n\
             starting with its key phrase in bold (2–5 words), no labels in front. Page references go in \"page\" and the lists, not in the table cells.\n\
             ANSWER with ONE JSON object: {{\"title\": \"…\", \"lead\": \"…\", \"actions\": [{{\"action\": \"…\", \"what\": \"…\", \"amount\": \"…\", \"why\": \"…\", \"page\": 4}}],\n\
             \"effect\": [\"…\"], \"reasons\": [\"…\"], \"risks\": [\"…\"], \"steps\": [\"…\"], \"explanation\": \"2–3 sentences in {ul} for the banker: what in the report led to these ideas and what to check before proposing them\"}}"
        ),
        "note" => format!(
            "TASK: draft a short note to the client for the page given (3–6 sentences, in {rl}, the bank's polite, calm tone, addressing the client as «Вы» in Russian).\n\
             Say what matters on the page in plain words with the key figures; mention the period; no jargon, no advice. The banker reviews it before it goes out.\n\
             Never mention errors, discrepancies or anything to be checked in the report — that is for the banker, not the client; never write \"page\" or \"стр.\" in the note.\n\
             ANSWER with ONE JSON object: {{\"text\": \"the note\", \"explanation\": \"one sentence in {ul} on what the note stresses\"}}"
        ),
        _ => format!(
            "TASK: prepare the banker for the meeting where this report is presented.\n\
             1. A summary of the report in {ul}: 3–5 sentences — the result for the period, what drove it, the state of the portfolio.\n\
             2. Where to pay attention: the 3–7 points the client will notice or ask about (good and bad), each with its page and why it matters.\n\
             3. A pitch in {rl} — how to present the report to the client: an opening line, 3–5 key messages in the order to say them (each with its page),\n\
                the questions the client is likely to ask with short honest answers from the report, and a closing line. Calm, factual, no promises.\n\
             4. An e-mail to the client in {rl}, ready to send with the report attached: a subject, then a greeting by name ([CLIENT]), 2–4 short paragraphs\n\
                with the main results in plain words and the key figures, an invitation to talk them through, and a polite sign-off (\"С уважением,\" / \"Kind regards,\") without a name.\n\
                Written to the client («Вы» in Russian), 100–180 words. Nothing internal: no page numbers, no questions to prepare for, no discrepancies, no advice or promises.\n\
                Paragraphs are separated by an empty line (\\n\\n).\n\
             THE BANKER'S PROPOSAL, if there is one: mention it in the summary (one sentence); in the pitch present it after the results as a message of its own (with its page),\n\
             add the questions it is likely to raise with honest answers, and in the e-mail say there is a proposal to discuss together (its main idea, no amounts\n\
             beyond what it states, no promises). Never present it as a fact or as advice from the report itself.\n\
             EMPHASIS: in every text, wrap the phrase that carries its main idea in **double asterisks** (a few words, at most one or two per sentence — e.g. \"**the capital fell by 124 million**, mostly through outflows\");\n\
             write figures as the report shows them, with their sign or unit (+10.7%, −209.8 млн): they are highlighted for the banker. No other markup.\n\
             ANSWER with ONE JSON object: {{\"summary\": \"…\", \"attention\": [{{\"page\": 7, \"tone\": \"good|bad|neutral\", \"text\": \"…\"}}],\n\
             \"pitch\": {{\"opening\": \"…\", \"messages\": [{{\"page\": 4, \"text\": \"…\"}}], \"questions\": [{{\"q\": \"…\", \"a\": \"…\"}}], \"closing\": \"…\"}},\n\
             \"email\": {{\"subject\": \"…\", \"body\": \"…\"}}}}"
        ),
    };
    format!("{common}\n\n{task}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_go_out_masked_and_come_back() {
        let mut m = Masker::new(&["Кирилл Шарихин".into()]);
        let out = m.mask("Счёт ***33540 · Премиальная карта Gold; ИИС 510IIS7, ISIN RU000A0JX0J2; клиент Кирилл Шарихин; +13,54%, 313 557 305, 20.10.2026, 2026");
        assert!(!out.contains("33540") && !out.contains("510IIS7") && !out.contains("RU000A0JX0J2") && !out.contains("Шарихин"), "{out}");
        for keep in ["+13,54%", "313 557 305", "20.10.2026", "2026", "Gold"] {
            assert!(out.contains(keep), "{keep} stays: {out}");
        }
        assert!(out.contains("[CLIENT]") && out.contains("[ID-1]"));
        let back = m.unmask("Для [CLIENT]: счёт [ID-1] и [ID-3].");
        assert_eq!(back, "Для Кирилл Шарихин: счёт ***33540 и RU000A0JX0J2.");
    }

    #[test]
    fn names_are_masked_as_whole_words() {
        let mut m = Masker::new(&[]);
        assert_eq!(m.name("Газпром", "INS"), "[INS-1]");
        assert_eq!(m.name("ОМС золото", "INS"), "[INS-2]");
        assert_eq!(m.name("Газпром", "INS"), "[INS-1]", "the same name, the same placeholder");
        let out = m.mask("Газпром −6.7%; ОМС золото +14.2%; Газпромнефть не та бумага");
        assert_eq!(out, "[INS-1] −6.7%; [INS-2] +14.2%; Газпромнефть не та бумага");
        assert_eq!(m.unmask("Лидер — [INS-2], аутсайдер — [INS-1]."), "Лидер — ОМС золото, аутсайдер — Газпром.");
    }

    #[test]
    fn figures_read_as_on_the_page() {
        assert_eq!(fmt(&serde_json::json!(0.10717), "percent"), "10.7%");
        assert_eq!(fmt(&serde_json::json!(-209800531.37), "currency"), "−209 800 531");
        assert_eq!(fmt(&serde_json::json!(437.5), "number"), "438");
        assert_eq!(fmt(&serde_json::json!(-0.027), "number"), "−0.027");
    }
}
