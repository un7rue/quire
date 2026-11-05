//! The banker's proposal: a page of the report, after the facts and before the legal text,
//! made from the banker's own notes — typed in, or read from a Word (.docx) or PowerPoint
//! (.pptx) file. Laid out as written, or written up by the AI from the notes and the report.
//! The brief, the pitch and the check read it like any other page.

use crate::model::{Dataset, Doc, Page, Source, Widget};
use serde_json::{json, Value as Json};

/// The pages' group: "PROPOSAL:<id>", how a proposal's pages are found again (to update or remove them).
/// A report may hold several proposals, each with its own id ("pr1", "pr2"…).
pub const GROUP: &str = "PROPOSAL";
/// Its data source: the proposed actions, one row each ("proposal_actions_<id>").
pub const DATASET: &str = "proposal_actions";

/// The group of proposal `id`'s pages.
pub fn group(id: &str) -> String {
    format!("{GROUP}:{id}")
}

/// The data source of proposal `id`'s actions.
pub fn dataset(id: &str) -> String {
    format!("{DATASET}_{id}")
}

/// Whether a page belongs to a proposal (any).
pub fn is_proposal(p: &Page) -> bool {
    p.group.starts_with(GROUP)
}

/// The proposal a page belongs to.
pub fn id_of(p: &Page) -> Option<&str> {
    p.group.strip_prefix(GROUP).and_then(|r| r.strip_prefix(':'))
}

/// The proposals in the report, in page order: (id, index of its first page).
pub fn list(doc: &Doc) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for (i, p) in doc.pages.iter().enumerate() {
        if let Some(id) = id_of(p) {
            if !out.iter().any(|(x, _)| x == id) {
                out.push((id.to_string(), i));
            }
        }
    }
    out
}

/// An id for a new proposal: the next free "prN".
pub fn next_id(doc: &Doc) -> String {
    let used = list(doc);
    (1..).map(|n| format!("pr{n}")).find(|id| !used.iter().any(|(x, _)| x == id)).unwrap()
}

/// The register id of proposal `id` in the report (on its first page's heading).
pub fn uid_of(doc: &Doc, id: &str) -> Option<String> {
    let g = group(id);
    doc.pages.iter().filter(|p| p.group == g).flat_map(|p| &p.widgets).find_map(|w| w.opts.get("proposalUid").and_then(|v| v.as_str()).map(String::from))
}

/// A proposal as its pages show it (for one made before the register: title, lead, actions from
/// its data, and the cards' points by their titles), with the notes kept on its heading.
pub fn spec_of(doc: &Doc, id: &str) -> (Json, String) {
    let g = group(id);
    let ws: Vec<&Widget> = doc.pages.iter().filter(|p| p.group == g).flat_map(|p| &p.widgets).collect();
    let head = ws.iter().find(|w| w.kind == "heading");
    let title = head.and_then(|w| w.opt_str("text")).unwrap_or("").to_string();
    let notes = head.and_then(|w| w.opt_str("proposalNotes")).unwrap_or("").to_string();
    let points = |names: &[&str]| -> Vec<String> {
        ws.iter().find(|w| w.kind == "text" && names.contains(&w.title.as_str())).and_then(|w| w.opt_str("text"))
            .map(|t| t.lines().map(|l| l.trim().trim_start_matches("- ").to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
    };
    // the lead: the large untitled text (not the footnote)
    let lead = ws.iter().filter(|w| w.kind == "text" && w.title.is_empty())
        .find(|w| w.style.pointer("/fonts/body/size").and_then(|v| v.as_f64()).unwrap_or(0.0) >= 10.0).and_then(|w| w.opt_str("text")).unwrap_or("").to_string();
    let ds = dataset(id);
    let actions: Vec<Json> = doc.datasets.iter().find(|d| d.id == ds).map(|d| match &d.source {
        Source::Inline { columns, rows } => rows.iter().map(|r| {
            let at = |k: &str| columns.iter().position(|c| c == k).and_then(|i| r.get(i)).cloned().unwrap_or(Json::Null);
            json!({"action": at("action"), "what": at("what"), "amount": at("amount"), "why": at("why")})
        }).collect(),
        _ => Vec::new(),
    }).unwrap_or_default();
    (json!({"title": title, "lead": lead, "actions": actions, "effect": points(&["Ожидаемый эффект", "Expected effect"]), "reasons": points(&["Почему сейчас", "Why now"]),
        "risks": points(&["Риски", "Risks"]), "steps": points(&["Следующие шаги", "Next steps"])}), notes)
}

/// Marks proposal `id` with its register id (on its heading).
pub fn set_uid(doc: &mut Doc, id: &str, uid: &str) {
    let g = group(id);
    if let Some(w) = doc.pages.iter_mut().filter(|p| p.group == g).flat_map(|p| p.widgets.iter_mut()).find(|w| w.kind == "heading") {
        if !w.opts.is_object() { w.opts = json!({}); }
        w.opts["proposalUid"] = json!(uid);
    }
}

/// The register ids of every proposal in the report.
pub fn uids(doc: &Doc) -> Vec<String> {
    list(doc).iter().filter_map(|(id, _)| uid_of(doc, id)).collect()
}

/// A proposal made when a report held only one ("PROPOSAL", "proposal_actions") takes an id.
pub fn migrate(doc: &mut Doc) {
    if !doc.pages.iter().any(|p| p.group == GROUP) {
        return;
    }
    let id = next_id(doc);
    let (g, ds) = (group(&id), dataset(&id));
    for p in doc.pages.iter_mut().filter(|p| is_proposal(p)) {
        p.group = g.clone();
        for w in &mut p.widgets {
            if let Some(b) = w.binding.as_mut().filter(|b| b.dataset == DATASET) {
                b.dataset = ds.clone();
            }
        }
    }
    if let Some(d) = doc.datasets.iter_mut().find(|d| d.id == DATASET) {
        d.id = ds;
    }
}
// (the table goes on over further pages when they don't fit; the AI is asked for far fewer)
const MAX_ACTIONS: usize = 20;
const MAX_POINTS: usize = 4;

// ------------------------------------------------------------------ reading files

/// The text of an uploaded file: paragraphs one per line, list items as "- …", a blank line
/// between a presentation's slides.
pub fn extract(name: &str, bytes: &[u8]) -> Result<String, String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "docx" => docx(bytes),
        "pptx" => pptx(bytes),
        "txt" | "md" | "markdown" | "text" => Ok(String::from_utf8_lossy(bytes).replace("\r\n", "\n")),
        "doc" | "ppt" => Err("Old .doc / .ppt files can't be read: save it as .docx or .pptx, or paste the text.".into()),
        _ => Err("Use a Word (.docx), PowerPoint (.pptx) or text file, or paste the text.".into()),
    }
}

fn zip_file(bytes: &[u8], path: &str) -> Result<String, String> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "The file can't be opened: is it a real .docx / .pptx?".to_string())?;
    let mut f = z.by_name(path).map_err(|_| format!("The file has no {path}."))?;
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|e| e.to_string())?;
    Ok(s)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#xA0;", " ").replace("&amp;", "&")
}

/// The texts of `<tag …>text</tag>` elements inside `xml`, joined.
fn texts(xml: &str, tag: &str) -> String {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        // "<w:t>" or "<w:t xml:space=…>", not "<w:tab/>" or "<w:tbl>"
        if !(after.starts_with('>') || after.starts_with(' ')) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push_str(&unescape(&body[..end]));
        rest = &body[end + close.len()..];
    }
    out
}

/// The elements `<tag>…</tag>` of `xml`, each whole.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i..];
        let next = after.as_bytes().get(open.len()).copied().unwrap_or(b' ');
        if next != b'>' && next != b' ' {
            rest = &rest[i + open.len()..];
            continue;
        }
        let Some(end) = after.find(&close) else { break };
        out.push(&after[..end + close.len()]);
        rest = &after[end + close.len()..];
    }
    out
}

fn docx(bytes: &[u8]) -> Result<String, String> {
    let xml = zip_file(bytes, "word/document.xml")?;
    let mut lines = Vec::new();
    for p in elements(&xml, "w:p") {
        let t = texts(p, "w:t");
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        let list = p.contains("<w:numPr") || p.contains("w:val=\"ListParagraph\"") || p.contains("w:val=\"ListBullet\"");
        let heading = p.contains("w:val=\"Heading") || p.contains("w:val=\"Title\"") || p.contains("w:val=\"Заголовок");
        lines.push(if list { format!("- {t}") } else if heading { format!("\n{t}") } else { t.to_string() });
    }
    if lines.is_empty() {
        return Err("The document has no text.".into());
    }
    Ok(lines.join("\n").trim().to_string())
}

fn pptx(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "The file can't be opened: is it a real .pptx?".to_string())?;
    // slides in their order (slide2 before slide10)
    let mut names: Vec<(usize, String)> = (0..z.len())
        .filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string()))
        .filter_map(|n| n.strip_prefix("ppt/slides/slide").and_then(|r| r.strip_suffix(".xml")).and_then(|k| k.parse().ok()).map(|k| (k, n.clone())))
        .collect();
    names.sort();
    let mut slides = Vec::new();
    for (_, n) in names {
        let mut s = String::new();
        z.by_name(&n).map_err(|e| e.to_string())?.read_to_string(&mut s).map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        for sp in elements(&s, "p:sp") {
            let title = sp.contains("type=\"title\"") || sp.contains("type=\"ctrTitle\"");
            for p in elements(sp, "a:p") {
                let t = texts(p, "a:t");
                let t = t.trim();
                if !t.is_empty() {
                    lines.push(if title { t.to_string() } else { format!("- {t}") });
                }
            }
        }
        if !lines.is_empty() {
            slides.push(lines.join("\n"));
        }
    }
    if slides.is_empty() {
        return Err("The presentation has no text.".into());
    }
    Ok(slides.join("\n\n"))
}

// ------------------------------------------------------------------ notes → proposal

/// A proposal laid out as written (no AI): the first short line is the title, the first
/// paragraph the lead, list items are the actions (a verb first, an amount where there is one),
/// and items under "Риски", "Шаги", "Почему"… go to those blocks.
pub fn plain(notes: &str, ru: bool) -> Json {
    let mut title = String::new();
    let mut lead: Vec<String> = Vec::new();
    let (mut actions, mut reasons, mut risks, mut steps, mut effect) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    #[derive(Clone, Copy, PartialEq)]
    enum To { Actions, Reasons, Risks, Steps, Effect }
    let mut to = To::Actions;
    let mut seen_body = false;
    for raw in notes.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let bullet = ["- ", "* ", "• ", "– ", "— "].iter().find_map(|b| line.strip_prefix(b)).or_else(|| {
            // "1. …" / "1) …"
            let d = line.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
            (d > 0 && matches!(line[d..].chars().next(), Some('.') | Some(')'))).then(|| line[d + 1..].trim_start())
        });
        // the first short line is the title, whatever it says
        if title.is_empty() && !seen_body && bullet.is_none() && line.chars().count() <= 90 {
            title = line.trim_end_matches(['.', ':']).to_string();
            continue;
        }
        // a heading: "Риски:" (a colon), or a line that is just a block's name
        let low = line.to_lowercase();
        let head = low.trim_end_matches(':').trim();
        let named = |keys: &[&str]| keys.iter().any(|k| if line.ends_with(':') { head.starts_with(k) } else { head == *k });
        let section = if bullet.is_some() || head.chars().count() > 40 { None }
            else if named(&["риски", "риск", "risks", "risk"]) { Some(To::Risks) }
            else if named(&["следующие шаги", "шаги", "next steps", "steps"]) { Some(To::Steps) }
            else if named(&["почему сейчас", "почему", "обоснование", "why now", "why", "rationale"]) { Some(To::Reasons) }
            else if named(&["ожидаемый эффект", "эффект", "expected effect", "effect", "impact"]) { Some(To::Effect) }
            else if named(&["предлагаем", "рекомендации", "действия", "proposal", "recommendations", "actions"]) { Some(To::Actions) }
            else { None };
        if let Some(s) = section {
            to = s;
            continue;
        }
        seen_body = true;
        match (bullet, to) {
            (Some(b), To::Actions) => actions.push(action_of(b)),
            (Some(b), To::Reasons) => reasons.push(b.to_string()),
            (None, To::Reasons) => reasons.push(line.to_string()),
            (Some(b), To::Risks) => risks.push(b.to_string()),
            (Some(b), To::Steps) => steps.push(b.to_string()),
            (Some(b), To::Effect) => effect.push(b.to_string()),
            (None, To::Risks) => risks.push(line.to_string()),
            (None, To::Steps) => steps.push(line.to_string()),
            (None, To::Effect) => effect.push(line.to_string()),
            (None, _) => lead.push(line.to_string()),
        }
    }
    if title.is_empty() {
        title = if ru { "Предложение".into() } else { "Proposal".into() };
    }
    json!({ "title": title, "lead": lead.join(" "), "actions": actions, "reasons": reasons, "risks": risks, "steps": steps, "effect": effect })
}

/// "Купить ОФЗ 26238 на 10 млн ₽ — снизить долю акций" → action, what, amount, why.
fn action_of(s: &str) -> Json {
    const VERBS: [&str; 22] = ["купить", "продать", "докупить", "сократить", "увеличить", "перевести", "открыть", "закрыть", "продлить", "вложить", "разместить", "зафиксировать",
        "реинвестировать", "buy", "sell", "add", "reduce", "move", "open", "close", "renew", "invest"];
    let (main, why) = match s.split_once(" — ").or_else(|| s.split_once(" - ")).or_else(|| s.split_once(": ")) {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (s.trim(), ""),
    };
    let first = main.split_whitespace().next().unwrap_or("");
    let verb = VERBS.iter().any(|v| first.to_lowercase() == *v);
    let rest = if verb { main[first.len()..].trim() } else { main };
    // an amount: "10 млн ₽", "5 000 000 руб.", "$2m", "15%"
    let amount = amount_in(rest);
    let what = amount.as_ref().map(|a| rest.replace(a.as_str(), "").replace(" на ", " ").replace("  ", " ").trim().trim_end_matches([',', ' ', '.']).trim().to_string()).unwrap_or_else(|| rest.to_string());
    let cap = |s: &str| { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
    json!({ "action": if verb { cap(first) } else { String::new() }, "what": what, "amount": amount.unwrap_or_default(), "why": why })
}

fn amount_in(s: &str) -> Option<String> {
    let words: Vec<&str> = s.split_whitespace().collect();
    for i in 0..words.len() {
        let w = words[i].trim_start_matches(['$', '€', '₽']);
        if !w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        // the number, its thousands groups ("5 000 000") and a unit after it
        let mut j = i + 1;
        while j < words.len() && words[j].len() == 3 && words[j].chars().all(|c| c.is_ascii_digit()) {
            j += 1;
        }
        let units = ["млн", "млрд", "тыс", "тыс.", "₽", "руб", "руб.", "рублей", "usd", "$", "€", "%", "m", "mn", "bn", "k", "долл."];
        while j < words.len() && units.iter().any(|u| words[j].trim_end_matches([',', '.']).eq_ignore_ascii_case(u.trim_end_matches('.'))) {
            j += 1;
        }
        let money = words[i].starts_with(['$', '€', '₽']) || j > i + 1 || words[i].ends_with('%');
        if money {
            return Some(words[i..j].join(" ").trim_end_matches([',', '.']).to_string());
        }
    }
    None
}

/// What the AI writes the proposal from: the banker's notes (masked) and the report's figures.
pub fn system(report_lang: &str, ui_lang: &str) -> String {
    let rl = if report_lang.starts_with("ru") { "Russian" } else { "English" };
    let ul = if ui_lang == "ru" { "Russian" } else { "English" };
    format!(
        "You help a private banker turn their notes into the proposal page of a client's portfolio report.\n\
         You get the banker's NOTES and the REPORT the page goes into (its figures, page by page).\n\
         Identities are masked: [CLIENT] is the client, [NAME-n] a person, [ACC-n] an account, [INS-n] an instrument, [ID-n] a code — keep them exactly as they are.\n\
         THE PROPOSAL IS THE BANKER'S: keep every action, instrument, amount and reason they give; don't add actions, instruments or amounts of your own,\n\
         and never change the banker's figures. You may: order and group the actions, write them clearly, give a short title and lead, and connect the\n\
         proposal to the report's own figures where the notes point to them (e.g. \"доля акций — 62% (стр. 4)\") — figures only as the report shows them.\n\
         No promises or guaranteed returns, no figures that are in neither the notes nor the report; an expected effect only as the banker states it,\n\
         or as simple arithmetic on the report's figures (say so). Polite, calm bank tone, to the client («Вы» in Russian). Write the page in {rl}.\n\
         It must fit one page — STRICT LENGTHS: title up to 60 characters; lead up to 250 characters (1–2 sentences); at most 5 actions, each with\n\
         \"action\" one short verb (Купить, Продать, Сократить, Перевести, Открыть, Продлить, Вложить), \"what\" up to 40 characters, \"amount\" up to 25\n\
         characters (\"до 13,5 млн ₽\"), \"why\" up to 50 characters; at most 3 points per list, each up to 90 characters, starting with a capital letter.\n\
         Empty lists are fine. Figures and page references belong in the lists and the lead, not in the table cells.\n\
         EMPHASIS (the page is read at a glance): in the lead wrap the one or two phrases that carry the main idea in **double asterisks**; every point of\n\
         \"effect\", \"reasons\", \"risks\" and \"steps\" starts with its key phrase in bold, 2–5 words, then the rest — e.g. \"**Доля акций — около 40%** (сейчас 62%, стр. 4)\",\n\
         \"**Ставка может снизиться** — доход по вкладу тоже\". No labels in front of points (not \"почему сейчас: …\", \"шаг: …\"); \"why\" in actions is a few plain words.\n\
         ANSWER with ONE JSON object: {{\"title\": \"…\", \"lead\": \"…\",\n\
         \"actions\": [{{\"action\": \"Купить|Продать|Перевести|… (one word)\", \"what\": \"the instrument or product\", \"amount\": \"as the banker wrote it, or empty\", \"why\": \"one short reason\"}}],\n\
         \"effect\": [\"…\"], \"reasons\": [\"…\"], \"risks\": [\"…\"], \"steps\": [\"…\"],\n\
         \"explanation\": \"one or two sentences in {ul} for the banker: what you did with the notes and anything you left out or couldn't place\"}}"
    )
}

// ------------------------------------------------------------------ the page

fn str_list(v: &Json, n: usize) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(tidy).filter(|s| !s.is_empty()).take(n).collect()
}

/// A point as the page shows it: no "почему сейчас:" style label in front, a capital first letter.
fn tidy(s: &str) -> String {
    let mut t = s.trim().trim_start_matches(['-', '•', ' ']).to_string();
    if let Some((head, rest)) = t.split_once(": ") {
        let h = head.trim_matches('*').to_lowercase();
        let labels = ["почему сейчас", "почему", "риск", "риски", "ближайший шаг", "следующий шаг", "шаг", "эффект", "why now", "risk", "next step", "step", "effect"];
        if labels.contains(&h.as_str()) {
            t = rest.trim().to_string();
        }
    }
    let mut c = t.chars();
    match c.next() {
        Some('*') => t,
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => t,
    }
}

/// Whether an action adds to a position (green) or takes from it (red); others stay neutral.
fn direction(action: &str) -> Option<bool> {
    let a = action.to_lowercase();
    // "Продлить" (renew) keeps what there is: neither buying nor selling
    if a.starts_with("продл") || a.starts_with("renew") || a.starts_with("extend") {
        return None;
    }
    const IN: [&str; 12] = ["куп", "докуп", "открыт", "увелич", "влож", "размест", "реинвест", "пополн", "buy", "add", "open", "invest"];
    const OUT: [&str; 10] = ["прод", "сократ", "закры", "зафикс", "вывест", "погас", "sell", "reduce", "close", "trim"];
    if OUT.iter().any(|k| a.starts_with(k)) { Some(false) } else if IN.iter().any(|k| a.starts_with(k)) { Some(true) } else { None }
}

fn w(v: Json) -> Widget {
    serde_json::from_value(v).unwrap_or_default()
}

/// A card with an accent: a tinted or plain background, an icon by the title, a coloured edge.
fn accent_card(fill: &str, ink: &str, icon: &str, edge: bool) -> Json {
    let mut s = json!({"card": true, "background": fill, "pad": 10,
        "fonts": {"title": {"size": 6.6, "weight": 700, "color": ink, "tracking": 0.08}, "body": {"size": 9.2, "color": "$ink", "lineHeight": 1.35}},
        "titleIcon": {"icon": icon, "fill": ink, "color": "$page"}});
    if edge {
        s["rule"] = json!("left");
        s["ruleColor"] = json!(ink);
    }
    s
}

/// At most `n` characters, cut at a word with "…"; a bold mark left open by the cut is dropped.
fn clip(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        return s.to_string();
    }
    let cut: String = s.chars().take(n).collect();
    let mut cut = match cut.rfind(' ') { Some(i) if i > n / 2 => cut[..i].to_string(), _ => cut };
    cut = cut.trim_end_matches([',', ';', ':', '—', '-', ' ', '(']).to_string();
    if cut.matches("**").count() % 2 == 1 {
        if let Some(i) = cut.rfind("**") { cut.replace_range(i..i + 2, ""); }
    }
    format!("{cut}…")
}

/// Text for a block with **bold** marks: a masked number ("***23990") would read as marks,
/// so its stars become look-alikes.
fn safe(s: &str) -> String {
    s.replace("***", "∗∗∗")
}

/// A table cell: no marks (tables don't show them), within its length.
fn cell(v: &Json, n: usize) -> Json {
    json!(safe(&clip(&v.as_str().unwrap_or("").replace("***", "∗∗∗").replace("**", ""), n)))
}

/// The proposal's pages (one, or two when `split`: the actions on the first, why / risks / steps on
/// the second), each as (title, widgets), and its actions' data source. `notes` are kept on the first
/// page, to edit it again later.
pub fn build(spec: &Json, notes: &str, ru: bool, split: bool, id: &str) -> (Vec<(String, Vec<Widget>)>, Dataset, String) {
    let t = |r: &str, e: &str| if ru { r.to_string() } else { e.to_string() };
    let title = spec["title"].as_str().filter(|s| !s.trim().is_empty()).map(|s| clip(&s.replace("**", ""), 70)).unwrap_or_else(|| t("Предложение", "Proposal"));
    let lead = safe(&clip(spec["lead"].as_str().unwrap_or(""), if split { 420 } else { 300 }));
    let actions: Vec<&Json> = spec["actions"].as_array().into_iter().flatten().take(MAX_ACTIONS).collect();
    let list = |k: &str| -> Vec<String> { str_list(&spec[k], if split { MAX_POINTS } else { 3 }).iter().map(|x| safe(&clip(x, if split { 160 } else { 120 }))).collect() };
    let effect = list("effect");
    // why now (the accent), risks (the "bad" colour), next steps (the "good" colour)
    let blocks: Vec<(String, Vec<String>, &str, &str)> = [(t("Почему сейчас", "Why now"), "reasons", "$coupon", "star"), (t("Риски", "Risks"), "risks", "$bad", "trend-down"), (t("Следующие шаги", "Next steps"), "steps", "$good", "flag")]
        .into_iter().map(|(h, k, c, i)| (h, list(k), c, i)).filter(|(_, v, _, _)| !v.is_empty()).collect();
    let bullets = |v: &[String]| v.iter().map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n");
    // lines a list takes in a card `wd` columns wide (about 5 characters a column at this size)
    let lines = |v: &[String], wd: u32| v.iter().map(|s| (s.replace("**", "").chars().count() as u32).div_ceil((wd.saturating_sub(1) * 5).max(20))).sum::<u32>();
    let heading = |text: &str, sub: &str, notes: Option<&str>| {
        let mut o = json!({"badge": t("Предложение", "Proposal"), "eyebrow": t("Для обсуждения", "For discussion"), "text": text, "subtitle": sub, "nav": false});
        if let Some(n) = notes { o["proposalNotes"] = json!(n); }
        w(json!({"kind": "heading", "variant": "account", "x": 0, "y": 0, "w": 24, "h": 4, "opts": o}))
    };
    let footnote = || w(json!({"kind": "text", "variant": "body", "x": 0, "y": 22, "w": 24, "h": 1, "style": {"fonts": {"body": {"size": 6.2, "color": "$muted"}}},
        "opts": {"text": t("Предложение подготовлено вашим банкиром для обсуждения и не является индивидуальной инвестиционной рекомендацией. Решения принимаются Вами после обсуждения условий и рисков.",
                           "This proposal is prepared by your banker for discussion and is not individual investment advice. Decisions are yours, after the terms and risks are discussed.")}}));

    let mut ws = vec![heading(&title, &t("Подготовлено вашим банкиром · {{end}}", "Prepared by your banker · {{end}}"), Some(notes))];
    // its entry in the register of proposals (see `proposals`)
    if let Some(uid) = spec["uid"].as_str() {
        ws[0].opts["proposalUid"] = json!(uid);
    }
    ws.push(w(json!({"kind": "divider", "variant": "ink", "x": 0, "y": 4, "w": 24, "h": 1})));
    let mut y = 5;
    // the lead, large, and the expected effect beside it on a green tint
    if !lead.is_empty() || !effect.is_empty() {
        let lead_w = if effect.is_empty() { 24 } else { 15 };
        // about 4 characters a column at 13 pt; a row holds about 1.6 lines
        // about 3.4 characters a column at 13 pt (bold runs wider); a row holds about 1.5 lines
        let lead_lines = (lead.replace("**", "").chars().count() as u32 * 10).div_ceil(lead_w * 34);
        let lead_rows = (lead_lines * 10).div_ceil(15) + 1;
        let eff_rows = if effect.is_empty() { 0 } else { (lines(&effect, 24 - 16) * 10).div_ceil(16) + 2 };
        let h = lead_rows.max(eff_rows).clamp(3, if split { 7 } else { 5 });
        if !lead.is_empty() {
            ws.push(w(json!({"kind": "text", "variant": "body", "x": 0, "y": y, "w": lead_w, "h": h,
                "opts": {"text": lead}, "style": {"fonts": {"body": {"size": 13, "color": "$ink", "lineHeight": 1.35}}}})));
        }
        if !effect.is_empty() {
            let x = if lead.is_empty() { 0 } else { 16 };
            ws.push(w(json!({"kind": "text", "variant": "body", "x": x, "y": y, "w": 24 - x, "h": h, "title": t("Ожидаемый эффект", "Expected effect"),
                "opts": {"text": bullets(&effect)}, "style": accent_card("$gainTint", "$good", "trend-up", false)})));
        }
        y += h + 1;
    }
    // the actions: buying in green, selling in red
    let rows: Vec<Vec<Json>> = actions.iter().enumerate().map(|(i, a)| {
        let why = a["why"].as_str().map(tidy).unwrap_or_default();
        vec![json!(i + 1), cell(&a["action"], 14), cell(&a["what"], 48), cell(&a["amount"], 30), cell(&json!(why), 70)]
    }).collect();
    let below = !split && !blocks.is_empty();
    if !rows.is_empty() {
        // the cards below keep at least 5 rows
        let room = 22u32.saturating_sub(y + if below { 5 } else { 0 });
        let h = ((rows.len() as u32 * 5).div_ceil(4) + 3).min(room.max(4));
        let any = |k: usize| rows.iter().any(|r| r[k].as_str().is_some_and(|s| !s.is_empty()));
        let mut order = vec!["n"];
        let mut measures = Vec::new();
        for (k, (id, field, label)) in [(1, ("m0", "action", t("Действие", "Action"))), (2, ("m1", "what", t("Что", "What"))), (3, ("m2", "amount", t("Сумма", "Amount"))), (4, ("m3", "why", t("Зачем", "Why")))] {
            if any(k) {
                measures.push(json!({"id": id, "field": field, "agg": "text", "label": label, "format": ""}));
                order.push(id);
            }
        }
        let pills: Vec<Json> = rows.iter().filter_map(|r| r[1].as_str()).filter(|a| !a.is_empty()).map(|a| match direction(a) {
            Some(true) => json!({"match": a, "fill": "$gainTint", "color": "$good"}),
            Some(false) => json!({"match": a, "fill": "$lossTint", "color": "$bad"}),
            None => json!({"match": a, "fill": "$chip", "color": "$iconInk"}),
        }).collect();
        ws.push(w(json!({"kind": "table", "variant": "lines", "x": 0, "y": y, "w": 24, "h": h, "title": t("Что мы предлагаем", "What we propose"),
            "binding": {"dataset": dataset(id), "dims": ["n"], "measures": measures, "window": "all", "filters": [], "sort": "data"},
            "style": {"fonts": {"table_cell": {"size": 9.4}, "table_head": {"size": 6.4}, "title": {"size": 9}}},
            // on two pages the actions may go on over further pages (the cards come after them); on one, they don't
            "opts": {"totals": false, "table": {"row_h": 26, "headLineWidth": 0.8, "paginate": split, "order": order, "cols": {
                "n": {"label": "№", "width": 3, "color": "$muted", "align": "left"},
                "m0": {"bold": true, "width": 13, "align": "left", "pill": {"on": true, "fill": "$chip", "color": "$iconInk", "rules": pills}},
                "m1": {"bold": true, "width": 30, "align": "left"}, "m2": {"bold": true, "width": 18, "align": "left"}, "m3": {"color": "$body", "align": "left"}}}}})));
        y += h;
    }
    // why now, risks, next steps: side by side, each with its colour and icon (below, or on a page of their own)
    let cards = |y: u32, room: u32, big: bool| -> Vec<Widget> {
        let n = blocks.len() as u32;
        let wd = (24 - (n - 1)) / n;
        let need = (blocks.iter().map(|(_, v, _, _)| lines(v, wd)).max().unwrap_or(1) * 10).div_ceil(16) + 3;
        let h = need.clamp(4, room.max(4));
        // on a page of their own the cards have room for larger type
        let card_style = |ink: &str, icon: &str| { let mut st = accent_card("$card", ink, icon, false); if big { st["fonts"]["body"]["size"] = json!(11); } st };
        blocks.iter().enumerate().map(|(i, (head, items, ink, icon))| {
            let x = i as u32 * (wd + 1);
            w(json!({"kind": "text", "variant": "body", "x": x, "y": y, "w": if i as u32 == n - 1 { 24 - x } else { wd }, "h": h, "title": head,
                "opts": {"text": bullets(items)}, "style": card_style(ink, icon)}))
        }).collect()
    };
    if below {
        ws.extend(cards(y, 22u32.saturating_sub(y), false));
    }
    ws.push(footnote());
    let page_title = if ru { format!("Предложение · {title}") } else { format!("Proposal · {title}") };
    let mut pages = vec![(page_title.clone(), ws)];
    if split && !blocks.is_empty() {
        let mut p2 = vec![heading(&title, &t("Почему сейчас, риски и следующие шаги", "Why now, risks and next steps"), None)];
        p2.push(w(json!({"kind": "divider", "variant": "ink", "x": 0, "y": 4, "w": 24, "h": 1})));
        p2.extend(cards(5, 16, true));
        p2.push(footnote());
        pages.push((format!("{page_title}{}", if ru { " (продолжение)" } else { " (continued)" }), p2));
    }
    let ds = Dataset {
        id: dataset(id),
        name: format!("{} · {title}", t("Предложение · действия", "Proposal · actions")),
        source: Source::Inline { columns: ["n", "action", "what", "amount", "why"].iter().map(|s| s.to_string()).collect(), rows },
        ..Default::default()
    };
    (pages, ds, title)
}

/// The proposal page uses only design views the report's theme has (one it lacks would keep
/// every other theme off the report's theme list): a missing one becomes the kind's first.
pub fn fit_views(doc: &mut Doc) {
    let styles = doc.theme.styles.clone();
    for p in doc.pages.iter_mut().filter(|p| p.group == GROUP) {
        for w in &mut p.widgets {
            let have = styles.get(&w.kind);
            if !w.variant.is_empty() && !have.is_some_and(|l| l.iter().any(|v| v.id == w.variant)) {
                w.variant = have.and_then(|l| l.first()).map(|v| v.id.clone()).unwrap_or_default();
            }
        }
    }
}

/// Where the proposal page goes: after the facts, before the legal text at the end
/// (the last plain page whose title or section says it is legal), else at the end.
pub fn insert_at(doc: &Doc) -> usize {
    let legal = |p: &Page| {
        let s = format!("{} {}", p.title, p.section).to_lowercase();
        s.contains("юридическ") || s.contains("legal") || s.contains("disclaimer") || s.contains("оговорк")
    };
    doc.pages.iter().rposition(|p| !is_proposal(p) && legal(p)).unwrap_or(doc.pages.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_lay_out_as_written() {
        let notes = "Снижение доли акций\n\nПредлагаем вернуть портфель к консервативному профилю.\n\n- Продать Банк ВТБ на 2 млн ₽ — доля акций выше профиля\n- Купить ОФЗ 26238 на 5 000 000 руб.\n\nРиски:\n- Ставка может снизиться\nСледующие шаги:\n- Встреча 5 ноября";
        let p = plain(notes, true);
        assert_eq!(p["title"], "Снижение доли акций");
        assert!(p["lead"].as_str().unwrap().starts_with("Предлагаем"));
        assert_eq!(p["actions"][0], json!({"action": "Продать", "what": "Банк ВТБ", "amount": "2 млн ₽", "why": "доля акций выше профиля"}));
        assert_eq!(p["actions"][1]["amount"], "5 000 000 руб");
        assert_eq!(p["actions"][1]["what"], "ОФЗ 26238");
        assert_eq!(p["risks"], json!(["Ставка может снизиться"]));
        assert_eq!(p["steps"], json!(["Встреча 5 ноября"]));
        // a first line that is also a block's name ("План") is the title, not a heading
        let p = plain("План\n\nКоротко о главном.\n\n- Купить фонд\n\nРиски\n- Рынок", true);
        assert_eq!((p["title"].as_str(), p["lead"].as_str()), (Some("План"), Some("Коротко о главном.")));
        assert_eq!(p["actions"].as_array().unwrap().len(), 1);
        assert_eq!(p["risks"], json!(["Рынок"]));
    }

    #[test]
    fn word_and_powerpoint_text() {
        use std::io::Write;
        let make = |files: &[(&str, &str)]| {
            let mut buf = std::io::Cursor::new(Vec::new());
            let mut z = zip::ZipWriter::new(&mut buf);
            for (n, s) in files {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(s.as_bytes()).unwrap();
            }
            z.finish().unwrap();
            buf.into_inner()
        };
        let doc = make(&[("word/document.xml", r#"<w:document><w:body><w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>План</w:t></w:r></w:p><w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t xml:space="preserve">Купить </w:t></w:r><w:r><w:t>ОФЗ &amp; фонд</w:t></w:r></w:p><w:p><w:r><w:tab/><w:t>Текст</w:t></w:r></w:p></w:body></w:document>"#)]);
        assert_eq!(extract("p.docx", &doc).unwrap(), "План\n- Купить ОФЗ & фонд\nТекст");
        let ppt = make(&[
            ("ppt/slides/slide10.xml", r#"<p:sld><p:sp><p:nvSpPr><p:ph type="title"/></p:nvSpPr><a:p><a:r><a:t>Второй</a:t></a:r></a:p></p:sp></p:sld>"#),
            ("ppt/slides/slide2.xml", r#"<p:sld><p:sp><p:ph type="title"/><a:p><a:r><a:t>Первый</a:t></a:r></a:p></p:sp><p:sp><a:p><a:r><a:t>Пункт</a:t></a:r></a:p></p:sp></p:sld>"#),
        ]);
        assert_eq!(extract("p.pptx", &ppt).unwrap(), "Первый\n- Пункт\n\nВторой");
        assert!(extract("p.doc", b"x").is_err());
    }
}
