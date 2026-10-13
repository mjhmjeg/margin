//! The two wire formats as plain data: what to send ([`Request`]) and how to read what comes back.
//! No I/O here, so this is tested without a network and builds for the browser too.

use serde_json::{Value, json};

use crate::{AssistantError, ChatReply, ChatRequest, Connector, Endpoint, Model, ProviderId, Role};

/// The `anthropic-version` header value the Messages API is called with.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Tokens Anthropic may write when a request doesn't say (the API requires a number).
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Most models one listing keeps (a provider with more is cut off, not an error).
pub const MAX_MODELS: usize = 5000;

/// An HTTP request to make. `headers` holds the key: never log a `Request`.
pub struct Request {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<String>,
}

fn headers(ep: &Endpoint, json_body: bool) -> Vec<(&'static str, String)> {
    let mut h = Vec::new();
    match ep.connector {
        Connector::Anthropic => {
            h.push(("anthropic-version", ANTHROPIC_VERSION.to_string()));
            if let Some(k) = &ep.api_key {
                h.push(("x-api-key", k.clone()));
            }
        }
        Connector::OpenAi => {
            if let Some(k) = &ep.api_key {
                h.push(("authorization", format!("Bearer {k}")));
            }
        }
    }
    h.push(("accept", "application/json".into()));
    if json_body {
        h.push(("content-type", "application/json".into()));
    }
    h
}

/// List the models. `after` is Anthropic's paging cursor (the previous page's `last_id`).
pub fn models_request(ep: &Endpoint, after: Option<&str>) -> Request {
    let url = match (ep.connector, after) {
        (Connector::Anthropic, Some(a)) => format!("{}?limit=1000&after_id={}", ep.url("models"), query_escape(a)),
        (Connector::Anthropic, None) => format!("{}?limit=1000", ep.url("models")),
        (Connector::OpenAi, _) => ep.url("models"),
    };
    Request { method: "GET", url, headers: headers(ep, false), body: None }
}

fn query_escape(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// One page of models, and Anthropic's cursor for the next page when there is one.
pub fn parse_models(connector: Connector, body: &str) -> Result<(Vec<Model>, Option<String>), AssistantError> {
    let v: Value = serde_json::from_str(body).map_err(|_| AssistantError::Protocol("the model list isn't JSON".into()))?;
    // OpenAI-compatible servers answer `{"data": [...]}`; a few local ones a bare array.
    let list = v.get("data").and_then(Value::as_array).or_else(|| v.as_array());
    let Some(list) = list else { return Err(AssistantError::Protocol("the answer has no model list".into())) };
    let mut out = Vec::new();
    for m in list.iter().take(MAX_MODELS) {
        let Some(raw) = m.get("id").and_then(Value::as_str) else { continue };
        // Gemini lists `models/gemini-…`; requests take the bare id.
        let id = raw.strip_prefix("models/").unwrap_or(raw).trim();
        if id.is_empty() {
            continue;
        }
        let name = m.get("display_name").or(m.get("name")).and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or(id);
        out.push(Model { id: id.to_string(), name: name.to_string() });
    }
    let next = match connector {
        Connector::Anthropic if v.get("has_more").and_then(Value::as_bool) == Some(true) => {
            v.get("last_id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
        }
        _ => None,
    };
    Ok((out, next))
}

fn role(r: Role) -> &'static str {
    match r {
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

/// A chat call; `stream` asks for server-sent events ([`StreamParser`]).
pub fn chat_request(ep: &Endpoint, req: &ChatRequest, stream: bool) -> Request {
    let messages: Vec<Value> = req.messages.iter().map(|m| json!({"role": role(m.role), "content": m.content})).collect();
    let (path, body) = match ep.connector {
        Connector::Anthropic => {
            let mut b = json!({"model": req.model, "max_tokens": req.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS), "messages": messages});
            if let Some(s) = req.system.as_ref().filter(|s| !s.is_empty()) {
                b["system"] = json!(s);
            }
            if stream {
                b["stream"] = json!(true);
            }
            ("messages", b)
        }
        Connector::OpenAi => {
            let mut all = Vec::new();
            if let Some(s) = req.system.as_ref().filter(|s| !s.is_empty()) {
                all.push(json!({"role": "system", "content": s}));
            }
            all.extend(messages);
            let mut b = json!({"model": req.model, "messages": all});
            if let Some(n) = req.max_tokens {
                // OpenAI's newer models refuse `max_tokens`; everyone else documents it.
                let field = if ep.provider == ProviderId::OpenAi { "max_completion_tokens" } else { "max_tokens" };
                b[field] = json!(n);
            }
            if req.json {
                b["response_format"] = json!({"type": "json_object"});
            }
            if stream {
                b["stream"] = json!(true);
            }
            ("chat/completions", b)
        }
    };
    let mut h = headers(ep, true);
    if stream {
        h.retain(|(k, _)| *k != "accept");
        h.push(("accept", "text/event-stream".into()));
    }
    Request { method: "POST", url: ep.url(path), headers: h, body: Some(body.to_string()) }
}

/// A complete (non-streamed) chat answer.
pub fn parse_chat(connector: Connector, body: &str) -> Result<ChatReply, AssistantError> {
    let v: Value = serde_json::from_str(body).map_err(|_| AssistantError::Protocol("the reply isn't JSON".into()))?;
    let model = v.get("model").and_then(Value::as_str).unwrap_or_default().to_string();
    match connector {
        Connector::Anthropic => {
            let Some(blocks) = v.get("content").and_then(Value::as_array) else {
                return Err(AssistantError::Protocol("the reply has no content".into()));
            };
            let text: String = blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect();
            let stop_reason = v.get("stop_reason").and_then(Value::as_str).map(str::to_string);
            Ok(ChatReply { text, model, stop_reason, json_refused: false })
        }
        Connector::OpenAi => {
            let Some(choice) = v.get("choices").and_then(Value::as_array).and_then(|c| c.first()) else {
                return Err(AssistantError::Protocol("the reply has no choices".into()));
            };
            let text = choice.pointer("/message/content").map(content_text).unwrap_or_default();
            let stop_reason = choice.get("finish_reason").and_then(Value::as_str).map(str::to_string);
            Ok(ChatReply { text, model, stop_reason, json_refused: false })
        }
    }
}

/// The answer text of an OpenAI-compatible `content`: a string, or (Mistral with reasoning, and
/// some others) a list of chunks, of which only `{"type": "text", "text": …}` are the answer;
/// `thinking` chunks are the model's reasoning and are left out.
/// Source: https://docs.mistral.ai/capabilities/reasoning/ (checked 2026-10-13).
pub fn content_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str).is_none_or(|t| t == "text" || t == "output_text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect(),
        _ => String::new(),
    }
}

/// Whether a refused request (`status`, `body`) was refused for asking for JSON mode, so it can
/// be sent again without it (LM Studio, for one, documents only `json_schema`).
pub fn refuses_json_mode(status: u16, body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    matches!(status, 400 | 422) && (b.contains("response_format") || b.contains("json_object") || b.contains("json mode"))
}

/// Whether a refused request was refused for its output-token number (too high for the model,
/// or the field not supported), so it can be sent again without it.
pub fn refuses_max_tokens(status: u16, body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    matches!(status, 400 | 422) && (b.contains("max_tokens") || b.contains("max_completion_tokens") || b.contains("max_output_tokens"))
}

/// The error for an HTTP status that isn't success. `body` is the provider's answer; its message
/// is shown (cut short), except for refused keys, whose messages may echo part of the key.
pub fn status_error(status: u16, body: &str, key: Option<&str>) -> AssistantError {
    if status == 401 || status == 403 {
        return AssistantError::BadKey;
    }
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let msg = v
        .pointer("/error/message")
        .or(v.get("message"))
        .or(v.pointer("/detail"))
        .or(v.get("error"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| match status {
            404 => "not found (check the address and the model)".into(),
            429 => "too many requests or out of credit; wait and try again".into(),
            500..=599 => "the provider had a problem; try again later".into(),
            _ => "request failed".into(),
        });
    AssistantError::Http { status, message: scrub(&msg, key) }
}

/// `text` cut to a readable length, with the key (if it is there at all) taken out.
pub fn scrub(text: &str, key: Option<&str>) -> String {
    let mut t: String = text.chars().filter(|c| !c.is_control()).take(300).collect();
    if let Some(k) = key.filter(|k| k.len() >= 4) {
        t = t.replace(k, "••••");
    }
    t
}

/// Reads a server-sent-event stream line by line and hands out text as it arrives.
pub struct StreamParser {
    connector: Connector,
    event: String,
    reply: ChatReply,
    done: bool,
}

impl StreamParser {
    pub fn new(connector: Connector) -> Self {
        StreamParser { connector, event: String::new(), reply: ChatReply::default(), done: false }
    }

    /// The stream said it is finished.
    pub fn done(&self) -> bool {
        self.done
    }

    /// Feed one line (without its line ending). Returns new text, if the line carried some.
    pub fn line(&mut self, line: &str) -> Result<Option<String>, AssistantError> {
        let line = line.trim_end_matches('\r');
        if let Some(e) = line.strip_prefix("event:") {
            self.event = e.trim().to_string();
            return Ok(None);
        }
        let Some(data) = line.strip_prefix("data:") else { return Ok(None) };
        let data = data.trim();
        if data == "[DONE]" {
            self.done = true;
            return Ok(None);
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else { return Ok(None) };
        if let Some(e) = v.get("error") {
            let msg = e.get("message").and_then(Value::as_str).or(e.as_str()).unwrap_or("the stream failed");
            return Err(AssistantError::Http { status: 200, message: scrub(msg, None) });
        }
        match self.connector {
            Connector::Anthropic => {
                let kind = v.get("type").and_then(Value::as_str).unwrap_or(self.event.as_str());
                match kind {
                    "message_start" => {
                        if let Some(m) = v.pointer("/message/model").and_then(Value::as_str) {
                            self.reply.model = m.to_string();
                        }
                    }
                    "content_block_delta" => {
                        if let Some(t) = v.pointer("/delta/text").and_then(Value::as_str) {
                            self.reply.text.push_str(t);
                            return Ok(Some(t.to_string()));
                        }
                    }
                    "message_delta" => {
                        if let Some(r) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
                            self.reply.stop_reason = Some(r.to_string());
                        }
                    }
                    "message_stop" => self.done = true,
                    _ => {}
                }
            }
            Connector::OpenAi => {
                if let Some(m) = v.get("model").and_then(Value::as_str) {
                    self.reply.model = m.to_string();
                }
                if let Some(choice) = v.get("choices").and_then(Value::as_array).and_then(|c| c.first()) {
                    if let Some(r) = choice.get("finish_reason").and_then(Value::as_str) {
                        self.reply.stop_reason = Some(r.to_string());
                    }
                    let t = choice.pointer("/delta/content").map(content_text).unwrap_or_default();
                    if !t.is_empty() {
                        self.reply.text.push_str(&t);
                        return Ok(Some(t));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Everything received.
    pub fn finish(self) -> ChatReply {
        self.reply
    }
}
