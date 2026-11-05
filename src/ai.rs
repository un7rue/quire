//! The AI assistants: model providers, API keys, the bank's data policy and
//! the request log, and the Data role's SQL assistant.
//!
//! Requests go out through the system's `curl` (no key ever reaches the web
//! UI or a command line: curl reads its settings, headers included, from
//! stdin). Keys live in the macOS Keychain — the person's own first, then the
//! bank-wide one an administrator set.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How a provider's API is spoken.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    Anthropic,
    /// OpenAI's chat completions (OpenAI, DeepSeek, Qwen's compatible mode)
    OpenAi,
    /// OpenAI-like chat, with an OAuth token exchanged from the authorization key
    GigaChat,
}

pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub style: Style,
    pub endpoint: &'static str,
    /// The model used until the person picks one (Test lists what the key can use).
    pub model: &'static str,
    pub tools: bool,
    pub vision: bool,
    /// Where the key is made, for the settings page.
    pub keys_at: &'static str,
}

pub const PROVIDERS: [Provider; 5] = [
    Provider { id: "claude", name: "Claude", style: Style::Anthropic, endpoint: "https://api.anthropic.com/v1", model: "claude-sonnet-5-5", tools: true, vision: true, keys_at: "console.anthropic.com" },
    Provider { id: "openai", name: "ChatGPT", style: Style::OpenAi, endpoint: "https://api.openai.com/v1", model: "gpt-4.1", tools: true, vision: true, keys_at: "platform.openai.com" },
    Provider { id: "deepseek", name: "DeepSeek", style: Style::OpenAi, endpoint: "https://api.deepseek.com", model: "deepseek-chat", tools: true, vision: false, keys_at: "platform.deepseek.com" },
    Provider { id: "qwen", name: "Qwen", style: Style::OpenAi, endpoint: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", model: "qwen-plus", tools: true, vision: true, keys_at: "dashscope (Alibaba Cloud Model Studio)" },
    Provider { id: "gigachat", name: "GigaChat", style: Style::GigaChat, endpoint: "https://gigachat.devices.sberbank.ru/api/v1", model: "GigaChat-2-Max", tools: true, vision: true, keys_at: "developers.sber.ru" },
];

pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// This computer's AI choices (no keys: those are in the Keychain).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct AiSettings {
    /// The provider assistants use ("" = the first one with a key).
    pub default: String,
    pub providers: BTreeMap<String, ProviderSettings>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ProviderSettings {
    pub model: String,
    /// Another endpoint (e.g. Qwen's mainland region, or a corporate proxy); "" = the provider's.
    pub endpoint: String,
    /// GigaChat's scope: GIGACHAT_API_PERS, _B2B or _CORP.
    pub scope: String,
}

/// The bank's rules, set by an administrator.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AiPolicy {
    /// What may leave the bank: "structure" (names and SQL only), "aggregates", "full".
    pub level: String,
    /// Providers people may use (empty = all).
    pub providers: Vec<String>,
    /// AI switched off altogether.
    pub off: bool,
    /// What a provider costs the bank, per million tokens sent and received (USD), for Admin → AI;
    /// a provider without one uses the app's list prices.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub prices: std::collections::BTreeMap<String, [f64; 2]>,
}

impl Default for AiPolicy {
    fn default() -> Self {
        AiPolicy { level: "structure".into(), providers: Vec::new(), off: false, prices: Default::default() }
    }
}

impl AiPolicy {
    pub fn allows(&self, id: &str) -> bool {
        !self.off && (self.providers.is_empty() || self.providers.iter().any(|p| p == id))
    }
}

// ------------------------------------------------------------------ keys

const ORG: &str = "org";

/// A key's name in the server's secrets file.
fn secret_name(id: &str, who: &str) -> String {
    format!("ai.{id}.{}", who.to_lowercase())
}

fn service(id: &str) -> String {
    format!("quire.ai.{id}")
}

/// A key may hold only what keys hold (and nothing that could break the Keychain command).
fn key_ok(k: &str) -> bool {
    !k.is_empty() && k.len() <= 4096 && k.chars().all(|c| c.is_ascii_graphic() && c != '"' && c != '\\' && c != '\'')
}

/// The stored key: the person's own (`who`), or the bank-wide one.
pub fn key_get(id: &str, who: &str) -> Option<String> {
    // the web server: its sealed secrets file
    if crate::secrets::enabled() {
        return crate::secrets::get(&secret_name(id, who));
    }
    if cfg!(not(target_os = "macos")) {
        return None;
    }
    let out = Command::new("/usr/bin/security").args(["find-generic-password", "-s", &service(id), "-a", who, "-w"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!k.is_empty()).then_some(k)
}

pub fn key_has(id: &str, who: &str) -> bool {
    if crate::secrets::enabled() {
        return crate::secrets::has(&secret_name(id, who));
    }
    if cfg!(not(target_os = "macos")) {
        return false;
    }
    Command::new("/usr/bin/security").args(["find-generic-password", "-s", &service(id), "-a", who]).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Saves a key (or removes it with None). The key goes to `security` on stdin, never as an argument.
pub fn key_set(id: &str, who: &str, key: Option<&str>) -> Result<(), String> {
    if crate::secrets::enabled() {
        let k = key.map(str::trim).filter(|k| !k.is_empty());
        if k.is_some_and(|k| !key_ok(k)) {
            return Err("That doesn’t look like an API key (it has spaces or quotes).".into());
        }
        return crate::secrets::set(&secret_name(id, who), k);
    }
    if cfg!(not(target_os = "macos")) {
        return Err("Keys are kept in the macOS Keychain; this computer has none.".into());
    }
    match key {
        None => {
            let _ = Command::new("/usr/bin/security").args(["delete-generic-password", "-s", &service(id), "-a", who]).output();
            Ok(())
        }
        Some(k) => {
            let k = k.trim();
            if !key_ok(k) {
                return Err("That doesn’t look like an API key (it has spaces or quotes).".into());
            }
            let mut child = Command::new("/usr/bin/security").arg("-i").stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
            let line = format!("add-generic-password -U -s \"{}\" -a \"{}\" -l \"Quire AI · {}\" -w \"{}\"\n", service(id), who.replace('"', ""), id, k);
            child.stdin.take().ok_or("Keychain unavailable")?.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            if out.status.success() && key_has(id, who) {
                Ok(())
            } else {
                Err(format!("The Keychain refused the key: {}", String::from_utf8_lossy(&out.stderr).trim()))
            }
        }
    }
}

/// The key a person uses: their own, else the bank's.
pub fn key_for(id: &str, who: &str) -> Option<(String, &'static str)> {
    key_get(id, who).map(|k| (k, "personal")).or_else(|| key_get(id, ORG).map(|k| (k, "bank")))
}

pub fn org() -> &'static str {
    ORG
}

// ------------------------------------------------------------------ http

/// The Russian Trusted Root and Sub CA (GigaChat's servers are signed by them).
const RU_CA: &str = concat!(include_str!("../resources/certs/russian_trusted_root_ca.cer"), "\n", include_str!("../resources/certs/russian_trusted_sub_ca.cer"));

/// A value for curl's config syntax.
fn cq(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

struct Http<'a> {
    tmp: &'a Path,
}

impl Http<'_> {
    /// One request; returns (status, body). Headers (keys included) travel on curl's stdin.
    fn send(&self, method: &str, url: &str, headers: &[(String, String)], body: Option<&str>, ru_ca: bool, timeout: u32) -> Result<(u16, String), String> {
        self.send_form(method, url, headers, body, &[], ru_ca, timeout)
    }

    /// The same, with multipart form fields (`name=value` or `name=@file;type=…`).
    #[allow(clippy::too_many_arguments)]
    fn send_form(&self, method: &str, url: &str, headers: &[(String, String)], body: Option<&str>, form: &[String], ru_ca: bool, timeout: u32) -> Result<(u16, String), String> {
        std::fs::create_dir_all(self.tmp).map_err(|e| e.to_string())?;
        let stamp = format!("{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let body_file = self.tmp.join(format!("ai-req-{stamp}.json"));
        let mut cfg = format!("url = {}\nrequest = {}\nsilent\nshow-error\nmax-time = {timeout}\nwrite-out = \"\\n%{{http_code}}\"\n", cq(url), cq(method));
        for (k, v) in headers {
            cfg.push_str(&format!("header = {}\n", cq(&format!("{k}: {v}"))));
        }
        if let Some(b) = body {
            std::fs::write(&body_file, b).map_err(|e| e.to_string())?;
            cfg.push_str(&format!("data-binary = {}\n", cq(&format!("@{}", body_file.display()))));
        }
        for f in form {
            cfg.push_str(&format!("form = {}\n", cq(f)));
        }
        let ca_file = self.tmp.join("russian_trusted_ca.pem");
        if ru_ca {
            if !ca_file.exists() {
                std::fs::write(&ca_file, RU_CA).map_err(|e| e.to_string())?;
            }
            cfg.push_str(&format!("cacert = {}\n", cq(&ca_file.display().to_string())));
        }
        let mut child = Command::new("curl").args(["-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("curl is not available: {e}"))?;
        child.stdin.take().ok_or("curl stdin")?.write_all(cfg.as_bytes()).map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string());
        let _ = std::fs::remove_file(&body_file);
        let out = out?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
        let code: u16 = code.trim().parse().unwrap_or(0);
        if code == 0 {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() { "No answer from the provider.".into() } else { format!("Could not reach the provider: {err}") });
        }
        Ok((code, body.to_string()))
    }
}

fn api_error(code: u16, body: &str) -> String {
    let msg = serde_json::from_str::<Json>(body).ok().and_then(|j| {
        j.pointer("/error/message").or_else(|| j.get("message")).or_else(|| j.pointer("/error")).and_then(|m| m.as_str().map(String::from))
    });
    let hint = match code {
        401 | 403 => " — check the API key in Settings → AI.",
        402 => " — the account has no credit left.",
        404 => " — the model or endpoint isn’t available to this key.",
        429 => " — too many requests or the quota is used up; try again shortly.",
        _ => "",
    };
    format!("The provider answered {code}: {}{hint}", msg.unwrap_or_else(|| body.chars().take(200).collect()))
}

// ------------------------------------------------------------------ chat

/// A conversation turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

impl Msg {
    pub fn user(s: impl Into<String>) -> Msg {
        Msg { role: "user".into(), content: s.into() }
    }
    pub fn assistant(s: impl Into<String>) -> Msg {
        Msg { role: "assistant".into(), content: s.into() }
    }
}

/// An earlier conversation as providers accept it: user and assistant turns alternating,
/// starting with the user, the last few only, each kept short.
pub fn tidy_history(h: &[Msg]) -> Vec<Msg> {
    let mut out: Vec<Msg> = Vec::new();
    for m in h.iter().rev().take(12).rev() {
        let role = if m.role == "assistant" { "assistant" } else { "user" };
        let text: String = m.content.chars().take(6000).collect();
        if text.trim().is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(l) if l.role == role => {
                l.content.push_str("\n\n");
                l.content.push_str(&text);
            }
            _ => out.push(Msg { role: role.into(), content: text }),
        }
    }
    while out.first().is_some_and(|m| m.role != "user") {
        out.remove(0);
    }
    // the new question follows: the history must end with the assistant
    while out.last().is_some_and(|m| m.role != "assistant") {
        out.pop();
    }
    out
}

/// A picture to show the model (a reference design): its media type and base64 data.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Image {
    pub media: String,
    pub data: String,
}

/// A provider ready to talk: its settings and key resolved.
pub struct Client<'a> {
    pub p: &'static Provider,
    pub model: String,
    /// Claude's effort (how long it thinks): "low", "medium", "high"… None: the model's default.
    pub effort: Option<&'static str>,
    endpoint: String,
    scope: String,
    key: String,
    http: Http<'a>,
    /// What went out and came back, for the request log: each request as text (pictures counted,
    /// not copied) and each answer; the tokens the provider counted (in, out).
    sent: std::cell::RefCell<Vec<String>>,
    received: std::cell::RefCell<Vec<String>>,
    tokens: std::cell::Cell<(u64, u64)>,
}

impl<'a> Client<'a> {
    pub fn new(p: &'static Provider, set: &ProviderSettings, key: String, tmp: &'a Path) -> Client<'a> {
        Client {
            p,
            model: if set.model.trim().is_empty() { p.model.to_string() } else { set.model.trim().to_string() },
            effort: None,
            endpoint: if set.endpoint.trim().is_empty() { p.endpoint.to_string() } else { set.endpoint.trim().trim_end_matches('/').to_string() },
            scope: if set.scope.is_empty() { "GIGACHAT_API_PERS".into() } else { set.scope.clone() },
            key,
            http: Http { tmp },
            sent: Default::default(),
            received: Default::default(),
            tokens: Default::default(),
        }
    }

    /// For the request log: what the person asked, every request as sent (already masked where
    /// the assistant masks), every answer as received, and the tokens counted.
    pub fn transcript(&self, prompt: &str) -> Json {
        let (i, o) = self.tokens.get();
        json!({"prompt": prompt, "sent": *self.sent.borrow(), "received": *self.received.borrow(), "tokens_in": i, "tokens_out": o})
    }

    fn ru(&self) -> bool {
        self.p.style == Style::GigaChat
    }

    /// GigaChat trades its authorization key for a 30-minute token.
    fn bearer(&self) -> Result<String, String> {
        if self.p.style != Style::GigaChat {
            return Ok(self.key.clone());
        }
        let rq = format!("{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}", rand32(), rand32() & 0xffff, rand32() & 0xfff, rand32() & 0xfff, (rand32() as u64) << 16 | (rand32() as u64 & 0xffff));
        let headers = vec![
            ("Authorization".into(), format!("Basic {}", self.key)),
            ("RqUID".into(), rq),
            ("Content-Type".into(), "application/x-www-form-urlencoded".into()),
            ("Accept".into(), "application/json".into()),
        ];
        let (code, body) = self.http.send("POST", "https://ngw.devices.sberbank.ru:9443/api/v2/oauth", &headers, Some(&format!("scope={}", self.scope)), true, 30)?;
        if code != 200 {
            return Err(api_error(code, &body));
        }
        serde_json::from_str::<Json>(&body).ok().and_then(|j| j["access_token"].as_str().map(String::from)).ok_or_else(|| "GigaChat gave no access token.".into())
    }

    fn headers(&self, bearer: &str) -> Vec<(String, String)> {
        match self.p.style {
            Style::Anthropic => vec![("x-api-key".into(), self.key.clone()), ("anthropic-version".into(), "2023-06-01".into()), ("Content-Type".into(), "application/json".into())],
            _ => vec![("Authorization".into(), format!("Bearer {bearer}")), ("Content-Type".into(), "application/json".into()), ("Accept".into(), "application/json".into())],
        }
    }

    /// The models this key can use.
    pub fn models(&self) -> Result<Vec<String>, String> {
        let bearer = self.bearer()?;
        let (code, body) = self.http.send("GET", &format!("{}/models", self.endpoint), &self.headers(&bearer), None, self.ru(), 30)?;
        if code != 200 {
            return Err(api_error(code, &body));
        }
        let j: Json = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let mut out: Vec<String> = j["data"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect()).unwrap_or_default();
        out.sort();
        Ok(out)
    }

    /// One answer to a conversation.
    pub fn chat(&self, system: &str, msgs: &[Msg], max_tokens: u32) -> Result<String, String> {
        self.chat_with(system, msgs, None, max_tokens)
    }

    /// GigaChat reads pictures from files uploaded first; answers the files' ids.
    fn upload(&self, bearer: &str, img: &Image) -> Result<String, String> {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.decode(img.data.as_bytes()).map_err(|_| "The picture is unreadable.".to_string())?;
        std::fs::create_dir_all(self.http.tmp).map_err(|e| e.to_string())?;
        let ext = if img.media.contains("png") { "png" } else { "jpg" };
        let file = self.http.tmp.join(format!("ai-img-{}.{ext}", std::process::id()));
        std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
        let headers = vec![("Authorization".into(), format!("Bearer {bearer}")), ("Accept".into(), "application/json".into())];
        let form = [format!("file=@{};type={}", file.display(), img.media), "purpose=general".to_string()];
        let r = self.http.send_form("POST", &format!("{}/files", self.endpoint), &headers, None, &form, true, 60);
        let _ = std::fs::remove_file(&file);
        let (code, body) = r?;
        if code != 200 {
            return Err(api_error(code, &body));
        }
        serde_json::from_str::<Json>(&body).ok().and_then(|j| j["id"].as_str().map(String::from)).ok_or_else(|| "GigaChat didn't take the picture.".into())
    }

    /// One answer to a conversation; `images` go with the message at their index.
    pub fn chat_with(&self, system: &str, msgs: &[Msg], images: Option<(&[Image], usize)>, max_tokens: u32) -> Result<String, String> {
        let images = images.filter(|(i, _)| !i.is_empty());
        if images.is_some() && !self.p.vision {
            return Err(format!("{} can't read pictures — pick Claude, ChatGPT, Qwen or GigaChat in the AI chip.", self.p.name));
        }
        // what goes out, as text (pictures counted, not copied)
        {
            let mut t = format!("SYSTEM\n{system}\n");
            for (i, m) in msgs.iter().enumerate() {
                let pics = images.filter(|(_, at)| *at == i).map(|(imgs, _)| format!(" [+{} picture(s)]", imgs.len())).unwrap_or_default();
                t.push_str(&format!("\n{}{pics}\n{}\n", m.role.to_uppercase(), m.content));
            }
            let t: String = t.chars().take(400_000).collect();
            self.sent.borrow_mut().push(t);
        }
        let bearer = self.bearer()?;
        let file_ids: Vec<String> = match (images, self.p.style) {
            (Some((imgs, _)), Style::GigaChat) => imgs.iter().map(|img| self.upload(&bearer, img)).collect::<Result<_, _>>()?,
            _ => Vec::new(),
        };
        // each message as the provider takes it, the pictures with their own
        let msg = |i: usize, m: &Msg| -> Json {
            match images.filter(|(_, at)| *at == i) {
                None => json!(m),
                Some((imgs, _)) => match self.p.style {
                    Style::Anthropic => {
                        let mut c: Vec<Json> = imgs.iter().map(|img| json!({"type": "image", "source": {"type": "base64", "media_type": img.media, "data": img.data}})).collect();
                        c.push(json!({"type": "text", "text": m.content}));
                        json!({"role": m.role, "content": c})
                    }
                    Style::GigaChat => json!({"role": m.role, "content": m.content, "attachments": file_ids}),
                    _ => {
                        let mut c = vec![json!({"type": "text", "text": m.content})];
                        c.extend(imgs.iter().map(|img| json!({"type": "image_url", "image_url": {"url": format!("data:{};base64,{}", img.media, img.data)}})));
                        json!({"role": m.role, "content": c})
                    }
                },
            }
        };
        let (url, mut body) = match self.p.style {
            // Claude: no sampling settings (newer models refuse temperature)
            // current Claude models think before answering (it can't be switched off); the thinking
            // counts against max_tokens, so give room for both — only what is used is billed
            Style::Anthropic => (
                format!("{}/messages", self.endpoint),
                {
                    let mut b = json!({"model": self.model, "max_tokens": max_tokens.max(16000), "system": system, "messages": msgs.iter().enumerate().map(|(i, m)| msg(i, m)).collect::<Vec<_>>()});
                    if let Some(e) = self.effort {
                        b["output_config"] = json!({"effort": e});
                    }
                    b
                },
            ),
            _ => {
                let mut all = vec![json!({"role": "system", "content": system})];
                all.extend(msgs.iter().enumerate().map(|(i, m)| msg(i, m)));
                (format!("{}/chat/completions", self.endpoint), json!({"model": self.model, "max_tokens": max_tokens, "temperature": 0.1, "messages": all}))
            }
        };
        // a model that refuses a setting gets the request again without it (or with its newer name)
        let mut text = String::new();
        for _ in 0..3 {
            let (code, t) = self.http.send("POST", &url, &self.headers(&bearer), Some(&body.to_string()), self.ru(), 300)?;
            if code == 200 {
                text = t;
                break;
            }
            let low = t.to_lowercase();
            let obj = body.as_object_mut().expect("request body");
            if code == 400 && (low.contains("effort") || low.contains("output_config")) && obj.remove("output_config").is_some() {
                continue;
            }
            if code == 400 && low.contains("temperature") && obj.remove("temperature").is_some() {
                continue;
            }
            if code == 400 && low.contains("max_tokens") && low.contains("max_completion_tokens") {
                if let Some(v) = obj.remove("max_tokens") {
                    obj.insert("max_completion_tokens".into(), v);
                    continue;
                }
            }
            return Err(api_error(code, &t));
        }
        if text.is_empty() {
            return Err("The provider refused the request settings.".into());
        }
        let j: Json = serde_json::from_str(&text).map_err(|e| format!("Unreadable answer: {e}"))?;
        // the tokens the provider counted (Anthropic: input/output; OpenAI-style: prompt/completion)
        let u = &j["usage"];
        let tin = u["input_tokens"].as_u64().or(u["prompt_tokens"].as_u64()).unwrap_or(0)
            + u["cache_read_input_tokens"].as_u64().unwrap_or(0) + u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
        let tout = u["output_tokens"].as_u64().or(u["completion_tokens"].as_u64()).unwrap_or(0);
        let (a, b) = self.tokens.get();
        self.tokens.set((a + tin, b + tout));
        let out = match self.p.style {
            Style::Anthropic => j["content"].as_array().map(|a| a.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")),
            _ => j.pointer("/choices/0/message/content").and_then(|c| c.as_str()).map(String::from),
        };
        if let Some(t) = out.as_deref() {
            self.received.borrow_mut().push(t.chars().take(200_000).collect());
        }
        let stop = j["stop_reason"].as_str().or(j.pointer("/choices/0/finish_reason").and_then(|v| v.as_str())).unwrap_or("").to_string();
        let kinds: Vec<&str> = j["content"].as_array().into_iter().flatten().filter_map(|b| b["type"].as_str()).collect();
        out.filter(|s| !s.trim().is_empty()).ok_or_else(|| match stop.as_str() {
            "refusal" | "content_filter" => "The model declined to answer this request.".into(),
            "" => "The model returned an empty answer.".into(),
            s => format!("The model returned an empty answer (stopped: {s}{}).", if kinds.is_empty() { String::new() } else { format!("; parts: {}", kinds.join(", ")) }),
        })
    }
}

fn rand32() -> u32 {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    u32::from_le_bytes(b)
}

// ------------------------------------------------------------------ log

/// One line per request: who, which provider and model, what for, how much was sent, and how it went.
pub fn log(dir: &Path, entry: Json) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("ai_log.jsonl")) {
        let _ = writeln!(f, "{entry}");
    }
}

thread_local! {
    /// The work session a request runs in (see `in_session`): written with it to the log.
    static SESSION: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// When the last request on this thread was logged: the answer carries it (`log_at`) so the
    /// person's thumbs up or down can find its request.
    static LAST_AT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// `in_session`, for an assistant's answer: it also carries when its request was logged (`log_at`).
pub fn in_session_stamped(id: &str, f: impl FnOnce() -> Result<Json, String>) -> Result<Json, String> {
    LAST_AT.with(|s| s.borrow_mut().clear());
    let r = in_session(id, f);
    let at = LAST_AT.with(|s| s.borrow().clone());
    r.map(|mut j| {
        if j.is_object() && !at.is_empty() {
            j["log_at"] = Json::String(at);
        }
        j
    })
}

/// Runs `f` (an AI request on its own thread) as part of work session `id`: the log then ties
/// the request to the session the person was in.
pub fn in_session<T>(id: &str, f: impl FnOnce() -> T) -> T {
    SESSION.with(|s| *s.borrow_mut() = id.to_string());
    let r = f();
    SESSION.with(|s| s.borrow_mut().clear());
    r
}

/// A log line with the request's transcript: what was asked, sent and received, and the tokens.
pub fn log_with(dir: &Path, client: &Client, prompt: &str, mut entry: Json) {
    LAST_AT.with(|s| *s.borrow_mut() = entry["at"].as_str().unwrap_or("").to_string());
    let sid = SESSION.with(|s| s.borrow().clone());
    if !sid.is_empty() {
        entry["session"] = Json::String(sid);
    }
    if let (Some(o), Some(t)) = (entry.as_object_mut(), client.transcript(prompt).as_object()) {
        for (k, v) in t {
            o.insert(k.clone(), v.clone());
        }
    }
    log(dir, entry);
}

pub fn read_log(dir: &Path, limit: usize) -> Vec<Json> {
    let text = std::fs::read_to_string(dir.join("ai_log.jsonl")).unwrap_or_default();
    let mut out: Vec<Json> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    out.reverse();
    out.truncate(limit);
    // the list without the texts (they can be long): the request and its answer come one by one
    for e in &mut out {
        if let Some(o) = e.as_object_mut() {
            let has = o.contains_key("sent");
            o.remove("sent");
            o.remove("received");
            o.insert("detail".into(), json!(has));
        }
    }
    out
}

/// One request in full: what was asked, sent and received (found by its time and person).
pub fn read_log_entry(dir: &Path, at: &str, user: &str) -> Option<Json> {
    let text = std::fs::read_to_string(dir.join("ai_log.jsonl")).unwrap_or_default();
    text.lines().filter_map(|l| serde_json::from_str::<Json>(l).ok()).find(|e| e["at"] == at && e["user"] == user)
}

// ------------------------------------------------------------------ the SQL assistant

/// What the SQL assistant may know of a database: tables, columns and types — no values.
pub fn schema_text(tables: &Json, cap: usize) -> String {
    let mut out = String::new();
    for (i, t) in tables.as_array().into_iter().flatten().enumerate() {
        let cols: Vec<String> = t["columns"].as_array().into_iter().flatten().map(|c| format!("{} {}", c["name"].as_str().unwrap_or(""), c["type"].as_str().unwrap_or(""))).collect();
        let name = match t["schema"].as_str() {
            Some(s) if !s.is_empty() && s != "main" => format!("{s}.{}", t["name"].as_str().unwrap_or("")),
            _ => t["name"].as_str().unwrap_or("").to_string(),
        };
        if i < cap {
            out.push_str(&format!("{} {name}({})\n", t["kind"].as_str().unwrap_or("table"), cols.join(", ")));
        } else {
            out.push_str(&format!("{} {name}\n", t["kind"].as_str().unwrap_or("table")));
        }
    }
    out
}

pub fn dialect(kind: &str) -> &'static str {
    match kind {
        "duckdb" => "DuckDB",
        "sqlite" => "SQLite",
        "postgres" => "PostgreSQL",
        "clickhouse" => "ClickHouse",
        _ => "SQL",
    }
}

/// The system prompt for writing, fixing or explaining SQL.
pub fn sql_system(dialect: &str, schema: &str, params: &[(String, String)], lang: &str) -> String {
    let ps = if params.is_empty() {
        "none".to_string()
    } else {
        params.iter().map(|(n, a)| format!(":{n} — {a}")).collect::<Vec<_>>().join("\n")
    };
    let language = if lang == "ru" { "Russian" } else { "English" };
    format!(
        "You are the SQL assistant of Quire, a client-reporting app used by a private bank's data team.\n\
         You write read-only {dialect} queries over the database described below. You know its structure only — never its values.\n\n\
         TABLES (name(column type, …)):\n{schema}\n\
         PARAMETERS the app fills in when the query runs (write them as :name; never inline values):\n{ps}\n\n\
         RULES\n\
         - One SELECT statement (WITH … allowed). No INSERT/UPDATE/DELETE/DDL, no PRAGMA/ATTACH/COPY, no trailing semicolon.\n\
         - Use only the tables and columns listed. Give computed columns short snake_case aliases.\n\
         - Format the SQL for reading: each clause (select, from, join, where, group by, order by) on its own line, columns and conditions indented — never one long line.\n\
         - Prefer clear CTEs over deep nesting. Filter by :client and the period parameters when the request is about a client or a period.\n\
         - Answer with ONE JSON object and nothing else: {{\"sql\": \"…\", \"explanation\": \"…\"}}. The explanation is 1–3 short sentences in {language} saying what the query returns — always {language}, whatever language the question or the data is in. SQL comments, if any, also in {language}.\n\
         - When asked only to explain a query, answer {{\"explanation\": \"…\"}} in {language}: what it returns, then how, in plain words (4–8 sentences, no SQL).\n\
         - When the request is a question about the database itself (where something is kept, how tables relate, what a column holds) rather than a request for data, answer {{\"explanation\": \"…\"}} with no \"sql\": name the tables and columns (in `backticks`) and how they join, 2–6 sentences in {language}.\n\
         - When asked to turn a query into a report source: replace literal values a report would vary (dates, client or account ids, currencies…) with the matching parameter above; keep literals that are part of the logic (status codes, constants); give every output column a clear snake_case alias; keep the result otherwise the same. Answer {{\"sql\": \"…\", \"explanation\": \"…\", \"name\": \"a short name for the source in {language}\", \"date_column\": \"the output column that dates each row, or empty\", \"changes\": [\"'2026-06-30' → :period_end\", …], \"left\": [\"a literal kept as it is — why (in {language})\", …]}}."
    )
}

/// The first JSON object in a model's answer (fences and chatter around it ignored).
pub fn parse_answer(s: &str) -> Option<Json> {
    let t = s.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t).trim();
    if let Ok(j) = serde_json::from_str::<Json>(t) {
        if j.is_object() {
            return Some(j);
        }
    }
    let (a, b) = (t.find('{')?, t.rfind('}')?);
    serde_json::from_str::<Json>(&t[a..=b]).ok().filter(|j| j.is_object())
}

/// Only a reading statement may run.
pub fn read_only(sql: &str) -> Result<String, String> {
    let s = sql.trim().trim_end_matches(';').trim().to_string();
    if s.is_empty() {
        return Err("The model returned no SQL.".into());
    }
    let low = s.to_lowercase();
    let first = low.split_whitespace().next().unwrap_or("");
    if !matches!(first, "select" | "with" | "from" | "(") {
        return Err(format!("Only SELECT queries are allowed (the model wrote “{first} …”)."));
    }
    let words: Vec<&str> = low.split(|c: char| !c.is_alphanumeric() && c != '_').collect();
    for bad in ["insert", "update", "delete", "drop", "alter", "create", "attach", "detach", "copy", "pragma", "truncate", "grant", "vacuum", "install", "load"] {
        if words.contains(&bad) {
            return Err(format!("The query contains “{bad}”, which isn’t allowed."));
        }
    }
    if s.contains(';') {
        return Err("Only one statement is allowed.".into());
    }
    Ok(s)
}

pub fn tmp_dir(base: &Path) -> PathBuf {
    base.join("ai")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_and_guards() {
        let j = parse_answer("Here:\n```json\n{\"sql\": \"select 1\", \"explanation\": \"one\"}\n```").unwrap();
        assert_eq!(j["sql"], "select 1");
        assert!(parse_answer("no json").is_none());
        assert!(read_only("with a as (select 1) select * from a;").is_ok());
        assert!(read_only("delete from t").is_err());
        assert!(read_only("select 1; drop table t").is_err());
        assert!(read_only("select * from t where note = 'x'").is_ok());
        assert!(read_only("select created_at from t").is_ok(), "a column named like a keyword is fine");
        assert_eq!(cq("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert!(key_ok("sk-ant-api03-AbC_def-123") && !key_ok("has space") && !key_ok("q\"uote"));
        let t = json!([{"schema": "main", "name": "clients", "kind": "table", "columns": [{"name": "id", "type": "varchar"}]}]);
        assert_eq!(schema_text(&t, 10), "table clients(id varchar)\n");
        let sys = sql_system("DuckDB", "table t(a int)\n", &[("client".into(), "the client".into())], "ru");
        assert!(sys.contains(":client — the client") && sys.contains("Russian"));
    }

    /// A stand-in provider: answers each request in turn with (status, body); returns what it received.
#[test]
    fn claude_gets_the_picture_as_an_image_block() {
        let tmp = std::env::temp_dir().join("quire_ai_img_test");
        let (url, seen) = fake(vec![(200, r#"{"content":[{"type":"text","text":"{}"}]}"#)]);
        let c = Client::new(provider("claude").unwrap(), &ProviderSettings { model: "m".into(), endpoint: url, scope: String::new() }, "k".into(), &tmp);
        let img = Image { media: "image/jpeg".into(), data: "AAAA".into() };
        c.chat_with("sys", &[Msg::user("look")], Some((std::slice::from_ref(&img), 0)), 100).unwrap();
        let req = seen.lock().unwrap()[0].clone();
        assert!(req.contains(r#""type":"image""#) && req.contains(r#""media_type":"image/jpeg""#) && req.contains(r#""data":"AAAA""#) && req.contains(r#""text":"look""#), "{req}");
    }

        fn fake(answers: Vec<(u16, &'static str)>) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let s2 = seen.clone();
        std::thread::spawn(move || {
            for (code, a) in answers {
                let (mut sock, _) = l.accept().unwrap();
                let mut got = Vec::new();
                let mut buf = vec![0u8; 65536];
                loop {
                    let n = sock.read(&mut buf).unwrap();
                    got.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&got).to_string();
                    if let Some(i) = t.find("\r\n\r\n") {
                        let len: usize = t.lines().find_map(|x| x.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
                        if got.len() >= i + 4 + len { break; }
                    }
                    if n == 0 { break; }
                }
                s2.lock().unwrap().push(String::from_utf8_lossy(&got).to_string());
                let r = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{a}", a.len());
                sock.write_all(r.as_bytes()).unwrap();
            }
        });
        (url, seen)
    }

    #[test]
    fn claude_gets_no_temperature_and_others_retry_without_it() {
        let tmp = std::env::temp_dir().join("quire_ai_fmt_test");
        // Claude: Anthropic's request shape, no sampling settings
        let (url, seen) = fake(vec![(200, r#"{"content":[{"type":"text","text":"{\"explanation\": \"ok\"}"}]}"#)]);
        let c = Client::new(provider("claude").unwrap(), &ProviderSettings { model: "claude-sonnet-5-5".into(), endpoint: url, scope: String::new() }, "sk-ant-x".into(), &tmp);
        assert_eq!(c.chat("sys", &[Msg::user("hi")], 100).unwrap(), "{\"explanation\": \"ok\"}");
        let req = seen.lock().unwrap()[0].clone();
        assert!(req.starts_with("POST /messages") && req.contains("x-api-key: sk-ant-x") && req.contains("anthropic-version: 2023-06-01"), "{req}");
        assert!(!req.contains("temperature") && req.contains("\"system\":\"sys\""), "{req}");
        // an OpenAI-style model that refuses temperature, then wants max_completion_tokens
        let (url, seen) = fake(vec![
            (400, r#"{"error":{"message":"Unsupported parameter: 'temperature' is not supported with this model."}}"#),
            (400, r#"{"error":{"message":"Unsupported parameter: 'max_tokens' is not supported with this model. Use 'max_completion_tokens' instead."}}"#),
            (200, r#"{"choices":[{"message":{"content":"done"}}]}"#),
        ]);
        let c = Client::new(provider("openai").unwrap(), &ProviderSettings { model: "m".into(), endpoint: url, scope: String::new() }, "sk-x".into(), &tmp);
        assert_eq!(c.chat("sys", &[Msg::user("hi")], 100).unwrap(), "done");
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].contains("\"temperature\"") && !reqs[1].contains("\"temperature\"") && reqs[2].contains("max_completion_tokens"));
    }


    #[test]
    fn history_alternates_and_ends_with_the_assistant() {
        let h = vec![Msg::assistant("stray"), Msg::user("a"), Msg::user("b"), Msg::assistant("x"), Msg::user("unanswered")];
        let t = tidy_history(&h);
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].role.as_str(), t[0].content.as_str()), ("user", "a\n\nb"));
        assert_eq!(t[1].role, "assistant");
        assert!(tidy_history(&[]).is_empty());
    }

}
