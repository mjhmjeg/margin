//! Optimo's assistant connectors (HANDOFF §9, S2–S3).
//!
//! People bring their own provider and API key. A provider is a [`Preset`]: a base URL, which of
//! the two wire formats it speaks ([`Connector`]), and whether a key is needed. Two connectors
//! cover every preset:
//!
//! * [`Connector::Anthropic`]: the Anthropic Messages API (`POST /messages`, `GET /models`,
//!   `x-api-key` + `anthropic-version` headers);
//! * [`Connector::OpenAi`]: OpenAI-compatible chat completions (`POST /chat/completions`,
//!   `GET /models`, `Authorization: Bearer`), used by Mistral, OpenAI, Gemini's OpenAI-compatible
//!   endpoint, OpenRouter, local servers (Ollama, LM Studio) and any other compatible endpoint.
//!
//! The wire formats ([`wire`]) are plain functions over JSON, so they build and run everywhere,
//! the browser included. The network ([`http`]) and the keychain ([`keys`]) exist on the desktop
//! only: their dependencies are not compiled for `wasm32`.
//!
//! Keys are never logged, never part of an error message and never in a `Debug` output
//! ([`Endpoint`] redacts it).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(not(target_arch = "wasm32"))]
pub mod http;
pub mod keys;
pub mod wire;

use std::time::Duration;

/// How long a call may take before it counts as failed.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// The base URL "On this Mac" starts with: Ollama's OpenAI-compatible endpoint. LM Studio's is
/// `http://localhost:1234/v1`.
pub const LOCAL_BASE_URL: &str = "http://localhost:11434/v1";

/// The wire format a provider speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connector {
    /// Anthropic Messages API.
    Anthropic,
    /// OpenAI-compatible chat completions.
    OpenAi,
}

/// The providers offered in Settings › Co-writer, in the order shown (S2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProviderId {
    Mistral,
    Anthropic,
    OpenAi,
    Gemini,
    OpenRouter,
    Local,
    Other,
}

/// A provider's fixed facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub id: ProviderId,
    pub connector: Connector,
    /// Where requests go; empty for [`ProviderId::Other`] (the person types it).
    pub base_url: &'static str,
    /// The person may change the base URL (local servers and other endpoints).
    pub editable_url: bool,
    /// A key is required (cloud providers). Local servers work without one.
    pub needs_key: bool,
}

// Base URLs and model-listing endpoints, checked against each provider's documentation on
// 2026-10-13:
// - Mistral: https://docs.mistral.ai/api/ — `https://api.mistral.ai/v1`, `GET /v1/models`,
//   `POST /v1/chat/completions`, `Authorization: Bearer`.
// - Anthropic: https://platform.claude.com/docs/en/api/models/list — `GET
//   https://api.anthropic.com/v1/models` (`limit` up to 1000, `has_more` / `last_id` paging),
//   headers `x-api-key` and `anthropic-version: 2023-06-01`; messages at `POST /v1/messages`.
// - OpenAI: https://developers.openai.com/api/reference/resources/models/methods/list — `GET
//   https://api.openai.com/v1/models`, `Authorization: Bearer`.
// - Google Gemini: https://ai.google.dev/gemini-api/docs/openai — OpenAI-compatible base
//   `https://generativelanguage.googleapis.com/v1beta/openai/`, `GET …/models`, Bearer key.
// - OpenRouter: https://openrouter.ai/docs/api/api-reference/models/get-models — `GET
//   https://openrouter.ai/api/v1/models`, Bearer key; chat at `/api/v1/chat/completions`.
// - Ollama: https://docs.ollama.com/api/openai-compatibility — `http://localhost:11434/v1/`,
//   `/v1/models` supported, key ignored. LM Studio: https://lmstudio.ai/docs/developer/openai-compat
//   — `http://localhost:1234/v1`, `GET /v1/models`.
const PRESETS: [Preset; 7] = [
    Preset { id: ProviderId::Mistral, connector: Connector::OpenAi, base_url: "https://api.mistral.ai/v1", editable_url: false, needs_key: true },
    Preset {
        id: ProviderId::Anthropic,
        connector: Connector::Anthropic,
        base_url: "https://api.anthropic.com/v1",
        editable_url: false,
        needs_key: true,
    },
    Preset { id: ProviderId::OpenAi, connector: Connector::OpenAi, base_url: "https://api.openai.com/v1", editable_url: false, needs_key: true },
    Preset {
        id: ProviderId::Gemini,
        connector: Connector::OpenAi,
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        editable_url: false,
        needs_key: true,
    },
    Preset {
        id: ProviderId::OpenRouter,
        connector: Connector::OpenAi,
        base_url: "https://openrouter.ai/api/v1",
        editable_url: false,
        needs_key: true,
    },
    Preset { id: ProviderId::Local, connector: Connector::OpenAi, base_url: LOCAL_BASE_URL, editable_url: true, needs_key: false },
    Preset { id: ProviderId::Other, connector: Connector::OpenAi, base_url: "", editable_url: true, needs_key: false },
];

impl ProviderId {
    /// Every provider, in the order Settings shows them; the first is pre-selected.
    pub const ALL: [ProviderId; 7] = [
        ProviderId::Mistral,
        ProviderId::Anthropic,
        ProviderId::OpenAi,
        ProviderId::Gemini,
        ProviderId::OpenRouter,
        ProviderId::Local,
        ProviderId::Other,
    ];

    /// The provider searches the web itself with the same key (Mistral's `web_search` tool,
    /// through its Conversations API): no other search service is involved on our side.
    pub fn web_search(self) -> bool {
        self == ProviderId::Mistral
    }

    /// The id stored in settings and used as the keychain account.
    pub fn id(self) -> &'static str {
        match self {
            ProviderId::Mistral => "mistral",
            ProviderId::Anthropic => "anthropic",
            ProviderId::OpenAi => "openai",
            ProviderId::Gemini => "gemini",
            ProviderId::OpenRouter => "openrouter",
            ProviderId::Local => "local",
            ProviderId::Other => "other",
        }
    }

    /// The name shown next to the radio button.
    pub fn label(self) -> &'static str {
        match self {
            ProviderId::Mistral => "Mistral",
            ProviderId::Anthropic => "Anthropic (Claude)",
            ProviderId::OpenAi => "OpenAI (ChatGPT)",
            ProviderId::Gemini => "Google (Gemini)",
            ProviderId::OpenRouter => "OpenRouter",
            ProviderId::Local => "On this Mac (Ollama, LM Studio)",
            ProviderId::Other => "Other (OpenAI-compatible)",
        }
    }

    pub fn from_id(id: &str) -> Option<ProviderId> {
        ProviderId::ALL.into_iter().find(|p| p.id() == id)
    }

    pub fn preset(self) -> Preset {
        PRESETS.into_iter().find(|p| p.id == self).unwrap_or(PRESETS[0])
    }
}

/// Where a call goes and with which key. `Debug` never shows the key.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The provider it belongs to (a few request fields differ between them).
    pub provider: ProviderId,
    pub connector: Connector,
    /// Without a trailing slash.
    pub base_url: String,
    pub api_key: Option<String>,
    /// A server on this computer: "nothing answering" means it isn't running, not "no network".
    pub local: bool,
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint")
            .field("provider", &self.provider)
            .field("connector", &self.connector)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("local", &self.local)
            .finish()
    }
}

/// The longest base URL accepted.
pub const MAX_URL_CHARS: usize = 2048;

impl Endpoint {
    /// The endpoint for `provider` with base URL `url` (the preset's when `None` or not editable)
    /// and `key`. Fails on a URL that isn't http(s), or plain http to another computer while a
    /// key would be sent (it would travel unencrypted).
    pub fn new(provider: ProviderId, url: Option<&str>, key: Option<String>) -> Result<Endpoint, AssistantError> {
        let preset = provider.preset();
        let raw = if preset.editable_url { url.map(str::trim).filter(|u| !u.is_empty()).unwrap_or(preset.base_url) } else { preset.base_url };
        let base_url = check_url(raw)?;
        let key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
        let loopback = is_loopback(&base_url);
        if base_url.starts_with("http://") && !loopback && key.is_some() {
            return Err(AssistantError::Setup("use an https:// address for a server on another computer: the key would travel unencrypted".into()));
        }
        Ok(Endpoint { provider, connector: preset.connector, base_url, api_key: key, local: provider == ProviderId::Local || loopback })
    }

    /// `base_url` + `/` + `path`.
    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}

/// A base URL as it is used: trimmed, http(s), no trailing slash, capped.
pub fn check_url(raw: &str) -> Result<String, AssistantError> {
    let u = raw.trim().trim_end_matches('/');
    if u.is_empty() {
        return Err(AssistantError::Setup("enter the server's address (for example https://example.com/v1)".into()));
    }
    if u.chars().count() > MAX_URL_CHARS || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AssistantError::Setup("that address isn't a valid URL".into()));
    }
    let lower = u.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    match rest {
        Some(r) if !r.is_empty() && !r.starts_with('/') => Ok(u.to_string()),
        _ => Err(AssistantError::Setup("the address must start with http:// or https://".into())),
    }
}

/// The host part of an http(s) URL, lower-case, without port.
pub fn host(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    let rest = lower.split_once("://").map_or(lower.as_str(), |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if let Some(v6) = authority.strip_prefix('[') {
        return v6.split(']').next().unwrap_or_default().to_string();
    }
    authority.split(':').next().unwrap_or_default().to_string()
}

/// Whether `url` points at this computer.
pub fn is_loopback(url: &str) -> bool {
    let h = host(url);
    h == "localhost" || h.ends_with(".localhost") || h == "::1" || h.starts_with("127.")
}

/// A model a provider offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// What requests name.
    pub id: String,
    /// What people read (the id when the provider gives no name).
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// A plain chat call.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    /// Required by Anthropic (4096 when `None`); left out for OpenAI-compatible servers when `None`
    /// (sent as `max_completion_tokens` to OpenAI, whose newer models refuse `max_tokens`).
    pub max_tokens: Option<u32>,
    /// Ask an OpenAI-compatible server for a JSON object (`response_format: json_object`). The
    /// Anthropic Messages API has no such switch; it is ignored there.
    pub json: bool,
}

/// What came back.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ChatReply {
    pub text: String,
    pub model: String,
    /// Why it stopped (`end_turn`, `stop`, `max_tokens`, `length`…), when the provider says.
    pub stop_reason: Option<String>,
    /// The server refused JSON mode (`ChatRequest::json`), so the call was made again without it.
    pub json_refused: bool,
}

/// A web page in a search answer: a reference the provider attached to its text, or a page its
/// search tool returned.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WebPage {
    pub title: String,
    pub url: String,
    /// As the search gave it, or empty.
    pub date: String,
}

/// A piece of a search answer, in order: text, or a reference to a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebPiece {
    Text(String),
    Ref(WebPage),
}

/// What a call with the provider's web search tool brought back.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WebReply {
    pub pieces: Vec<WebPiece>,
    /// The pages the search returned (what the references may point at).
    pub searched: Vec<WebPage>,
    pub model: String,
}

impl WebReply {
    /// The answer's text without its references.
    pub fn text(&self) -> String {
        self.pieces.iter().filter_map(|p| if let WebPiece::Text(t) = p { Some(t.as_str()) } else { None }).collect()
    }
}

/// Why a call failed, in words a person can act on. Never contains the key.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AssistantError {
    /// No key stored for a provider that needs one.
    #[error("add an API key first")]
    NoKey,
    /// The provider refused the key (401/403).
    #[error("the key was refused: check it, or paste it again")]
    BadKey,
    /// Nothing reachable (DNS, offline, connection refused on a remote host).
    #[error("couldn't reach {0}: check the internet connection")]
    NoNetwork(String),
    /// A local server isn't answering.
    #[error("nothing is answering at {0}: start Ollama or LM Studio (and load a model), then try again")]
    NotRunning(String),
    /// No answer in time.
    #[error("no answer within {0} seconds")]
    Timeout(u64),
    /// Another HTTP error, with the provider's own message.
    #[error("the provider answered {status}: {message}")]
    Http { status: u16, message: String },
    /// An answer that isn't what the API documents.
    #[error("unexpected answer: {0}")]
    Protocol(String),
    /// A setting that can't work (address, key format).
    #[error("{0}")]
    Setup(String),
    /// The person stopped it.
    #[error("stopped")]
    Stopped,
    /// Not available here (browser build, no keychain).
    #[error("{0}")]
    Unsupported(String),
}

/// The text a key field shows: `••••` and the key's last four characters.
pub fn masked(key: &str) -> String {
    let chars: Vec<char> = key.trim().chars().collect();
    let tail: String = chars.iter().skip(chars.len().saturating_sub(4)).collect();
    format!("••••{tail}")
}

/// The longest key accepted.
pub const MAX_KEY_CHARS: usize = 1024;

/// A pasted key as it is stored: trimmed; refused when empty, too long, or with spaces or control
/// characters inside (a paste that caught more than the key).
pub fn clean_key(raw: &str) -> Result<String, AssistantError> {
    let k = raw.trim();
    if k.is_empty() {
        return Err(AssistantError::Setup("paste the key first".into()));
    }
    if k.chars().count() > MAX_KEY_CHARS || k.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AssistantError::Setup("that doesn't look like an API key (it has spaces or is too long)".into()));
    }
    Ok(k.to_string())
}

#[cfg(test)]
mod tests;
