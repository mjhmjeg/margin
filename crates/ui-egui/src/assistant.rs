//! Settings › Assistant (HANDOFF §9): what the person decided about assistants, and the
//! bookkeeping for assistants that connect from outside Optimo (Claude Desktop, Claude Code, any
//! MCP client through the control channel).
//!
//! The rule for outside assistants lives in the engine (`wordcraft_engine::assistant`): their
//! edits are tracked changes under [`AssistantPrefs::name`], and they cannot accept, reject or
//! loosen it. [`crate::control::handle_outside`] applies it to every request a connection sends.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wordcraft_assistant::ProviderId;
use wordcraft_engine::cmd::file::MAX_AUTHOR_CHARS;

/// The name assistants wrote under before names were derived from the model (A3/A4). It stays a
/// legacy assistant name, so suggestions made then are still recognised.
pub const DEFAULT_NAME: &str = "Claude";

/// The name when there is neither a typed name, nor a model, nor a provider name.
pub const FALLBACK_NAME: &str = "Assistant";

/// Most legacy names remembered.
pub const MAX_PAST_NAMES: usize = 50;

/// Longest name derived from a model id.
pub const MAX_MODEL_NAME_CHARS: usize = 40;

/// Words always written in capitals in a model name.
const ACRONYMS: [&str; 8] = ["gpt", "glm", "llm", "ai", "r1", "oss", "vl", "qwq"];

/// `name` as assistants write under it: with the "AI · " prefix (never doubled; "Assistant" when
/// blank).
pub fn with_prefix(name: &str) -> String {
    let n = clean_name(name);
    wordcraft_engine::authorship::with_ai_prefix(if n.is_empty() { FALLBACK_NAME } else { &n })
}

/// A provider's short name, for the author name when no model is chosen.
pub fn provider_name(p: ProviderId) -> &'static str {
    match p {
        ProviderId::Mistral => "Mistral",
        ProviderId::Anthropic => "Anthropic",
        ProviderId::OpenAi => "OpenAI",
        ProviderId::Gemini => "Gemini",
        ProviderId::OpenRouter => "OpenRouter",
        ProviderId::Local => "Local model",
        ProviderId::Other => FALLBACK_NAME,
    }
}

/// A model id as people read it: `claude-opus-5-5` → "Claude Opus 5.5", `gpt-4-1` → "GPT 4.1",
/// `mistralai/mistral-large-latest` → "Mistral Large", `gemini-2.5-pro` → "Gemini 2.5 Pro",
/// `llama3.1:8b` → "Llama 3.1 8B". A vendor prefix before `/` goes; so do `latest` and date stamps
/// (`20250929`, `2025-09-29`, `2411`). Words are split on `-`, `_` and spaces and capitalised,
/// known acronyms upper-cased, numbers next to each other joined with "." (a version), a size
/// like `8b` upper-cased, and a word followed by its version (`llama3.1`) split. `None` when
/// nothing is left. Capped at [`MAX_MODEL_NAME_CHARS`].
pub fn model_display_name(id: &str) -> Option<String> {
    let id = id.trim();
    let id = id.rsplit('/').next().unwrap_or(id);
    // Ollama's `name:tag`: the tag is another word.
    let raw: Vec<String> = id
        .split([':', '-', '_', ' '])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().filter(|c| !c.is_control()).collect::<String>())
        .collect();
    let digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    // Drop `latest` and date stamps: yyyymmdd, yyyy mm dd, yymm (23xx–39xx, month 01–12).
    let mut toks: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some(t) = raw.get(i) {
        let lower = t.to_ascii_lowercase();
        if lower == "latest" {
            i += 1;
            continue;
        }
        if digits(t) && t.len() == 8 && (t.starts_with("19") || t.starts_with("20")) {
            i += 1;
            continue;
        }
        if digits(t) && t.len() == 4 && (t.starts_with("19") || t.starts_with("20")) {
            // yyyy-mm-dd: the year and the two parts after it.
            let mm = raw.get(i + 1).is_some_and(|x| digits(x) && x.len() == 2);
            let dd = raw.get(i + 2).is_some_and(|x| digits(x) && x.len() == 2);
            if mm && dd {
                i += 3;
                continue;
            }
        }
        if digits(t) && t.len() == 4 && i > 0 {
            let yy: u32 = t.get(..2).and_then(|x| x.parse().ok()).unwrap_or(0);
            let mm: u32 = t.get(2..).and_then(|x| x.parse().ok()).unwrap_or(0);
            if (23..=39).contains(&yy) && (1..=12).contains(&mm) {
                i += 1;
                continue;
            }
        }
        toks.push(t.clone());
        i += 1;
    }
    // A word with its version glued on (`llama3.1`, `qwen2`): two words. Not `4o`, `o3`, `r1`.
    let mut split: Vec<String> = Vec::new();
    for t in toks {
        let lower = t.to_ascii_lowercase();
        let letters = t.chars().take_while(|c| c.is_alphabetic()).count();
        let rest: String = t.chars().skip(letters).collect();
        if letters >= 3 && !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.') && !ACRONYMS.contains(&lower.as_str()) {
            split.push(t.chars().take(letters).collect());
            split.push(rest);
        } else {
            split.push(t);
        }
    }
    // Numbers next to each other are one version: 5 5 → 5.5.
    let mut words: Vec<String> = Vec::new();
    for t in split {
        if digits(&t)
            && let Some(last) = words.last_mut()
            && last.chars().all(|c| c.is_ascii_digit() || c == '.')
            && last.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            last.push('.');
            last.push_str(&t);
            continue;
        }
        words.push(t);
    }
    let shown: Vec<String> = words
        .into_iter()
        .map(|w| {
            let lower = w.to_lowercase();
            if ACRONYMS.contains(&lower.as_str()) {
                return lower.to_uppercase();
            }
            // A size: 8b, 70b, 1.5b, 128k.
            let n = lower.chars().take_while(|c| c.is_ascii_digit() || *c == '.').count();
            let unit: String = lower.chars().skip(n).collect();
            if n > 0 && matches!(unit.as_str(), "b" | "m" | "k" | "t") {
                return lower.to_uppercase();
            }
            let mut c = w.chars();
            match c.next() {
                Some(f) if f.is_alphabetic() => f.to_uppercase().chain(c).collect(),
                Some(f) => std::iter::once(f).chain(c).collect(),
                None => String::new(),
            }
        })
        .filter(|w| !w.is_empty())
        .collect();
    let name: String = shown.join(" ").chars().take(MAX_MODEL_NAME_CHARS).collect();
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AssistantPrefs {
    /// S4: the name typed in Settings (optional; without the "AI · " prefix, which is always
    /// added). Empty: derived from the model ([`AssistantPrefs::name`]).
    pub author_name: String,
    /// Names assistants wrote under on this Mac, for suggestions made without the "AI · " prefix
    /// (before it existed): recognised as an assistant's by the legacy rule. Never shown.
    pub past_names: Vec<String>,
    /// S6: assistants outside Optimo may use the control channel (which only exists when the app
    /// was started with `--control`). On by default, as the channel itself is an opt-in.
    pub allow_outside: bool,
    /// S1: "Let an assistant work in Optimo". Off by default; while off nothing is sent anywhere.
    pub enabled: bool,
    /// S2: the provider id (`wordcraft_assistant::ProviderId::id`); Mistral until chosen.
    pub provider: String,
    /// The model chosen per provider id.
    pub models: BTreeMap<String, String>,
    /// The base URL per provider id, for those where it can be changed (On this Mac, Other).
    pub base_urls: BTreeMap<String, String>,
    /// S5: "Add and reply to comments". Off: the built-in assistant's comment edits are dropped
    /// and assistants outside Optimo are refused `review.newComment` / `review.reply`.
    pub comments: bool,
    /// S5: "Read files in these folders (read-only)": the only places an assistant may read files
    /// from (the built-in one for "Check facts against a file…", the outside ones for any
    /// command that reads a path). Absolute, links followed. Empty: no file access.
    pub folders: Vec<String>,
    // API keys are not here: they live in the system keychain (`crate::assistant_link`).
}

impl Default for AssistantPrefs {
    fn default() -> Self {
        AssistantPrefs {
            author_name: String::new(),
            past_names: Vec::new(),
            allow_outside: true,
            enabled: false,
            provider: ProviderId::Mistral.id().into(),
            models: BTreeMap::new(),
            base_urls: BTreeMap::new(),
            comments: true,
            folders: Vec::new(),
        }
    }
}

/// Most folders the list holds.
pub const MAX_FOLDERS: usize = 20;
/// Longest folder path accepted.
pub const MAX_FOLDER_CHARS: usize = 1024;

/// A folder as the list stores it: it must exist and be a folder, not the top of the file
/// system; absolute, links followed.
pub fn check_folder(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().count() > MAX_FOLDER_CHARS || raw.chars().any(char::is_control) {
        return Err("`addFolder` isn't a folder path".into());
    }
    let path = std::fs::canonicalize(raw).map_err(|e| format!("can't use {raw}: {e}"))?;
    if !path.is_dir() {
        return Err(format!("{raw} is not a folder"));
    }
    if path.parent().is_none() {
        return Err("the top of the file system can't be shared: pick a folder inside it".into());
    }
    Ok(path.to_string_lossy().into_owned())
}

/// An author name as the engine stores it: trimmed, no control characters, capped.
pub fn clean_name(raw: &str) -> String {
    let n: String = raw.trim().chars().filter(|c| !c.is_control()).take(MAX_AUTHOR_CHARS).collect();
    n.trim_end().to_string()
}

impl AssistantPrefs {
    /// The author name assistants write under: "AI · " + the typed name, or else the selected
    /// model's display name, the provider's name, or "Assistant".
    pub fn name(&self) -> String {
        crate::assistant::with_prefix(&self.base_name())
    }

    /// [`AssistantPrefs::name`] without the prefix.
    pub fn base_name(&self) -> String {
        let typed = clean_name(&self.author_name);
        let typed = typed.strip_prefix(wordcraft_engine::authorship::AI_PREFIX).unwrap_or(&typed).trim().to_string();
        if typed.is_empty() { self.derived_name() } else { typed }
    }

    /// The name when none is typed: the model's display name, else the provider's, else
    /// "Assistant".
    pub fn derived_name(&self) -> String {
        let p = self.provider_id();
        self.model(p).and_then(model_display_name).unwrap_or_else(|| provider_name(p).to_string())
    }

    /// Every name the legacy rule recognises: the names remembered, the typed name as it was
    /// written before the prefix, and the old default.
    pub fn legacy_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let typed = clean_name(&self.author_name);
        for n in self.past_names.iter().map(|n| clean_name(n)).chain([typed, DEFAULT_NAME.to_string()]) {
            if !n.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
                out.push(n);
            }
        }
        out
    }

    /// Remember `name` (an author an assistant wrote under) for the legacy rule.
    pub fn remember_name(&mut self, name: &str) {
        let n = clean_name(name);
        if n.is_empty() || self.past_names.iter().any(|p| p.eq_ignore_ascii_case(&n)) {
            return;
        }
        if self.past_names.len() >= MAX_PAST_NAMES {
            self.past_names.remove(0);
        }
        self.past_names.push(n);
    }

    /// The folders in force: the list without anything a hand-edited `ui.json` could have put there
    /// that the Permissions list would not take (the top of the file system, control characters,
    /// more than [`MAX_FOLDERS`]).
    pub fn allowed_folders(&self) -> Vec<String> {
        self.folders
            .iter()
            .filter(|f| {
                let p = std::path::Path::new(f.as_str());
                p.is_absolute() && p.parent().is_some() && f.chars().count() <= MAX_FOLDER_CHARS && !f.chars().any(char::is_control)
            })
            .take(MAX_FOLDERS)
            .cloned()
            .collect()
    }

    /// The chosen provider (Mistral when the setting names none we know).
    pub fn provider_id(&self) -> ProviderId {
        ProviderId::from_id(&self.provider).unwrap_or(ProviderId::Mistral)
    }

    /// The model chosen for `p`, if any.
    pub fn model(&self, p: ProviderId) -> Option<&str> {
        self.models.get(p.id()).map(|m| m.trim()).filter(|m| !m.is_empty())
    }

    /// The base URL for `p`: the person's for editable presets, else the preset's.
    pub fn base_url(&self, p: ProviderId) -> String {
        let preset = p.preset();
        match self.base_urls.get(p.id()).map(|u| u.trim()).filter(|u| !u.is_empty()) {
            Some(u) if preset.editable_url => u.to_string(),
            _ => preset.base_url.to_string(),
        }
    }

    /// The settings as commands report them. `has_key` / `usable` come from the keychain cache;
    /// the key itself is never part of this.
    pub fn to_json(&self, has_key: bool, usable: bool) -> Value {
        let p = self.provider_id();
        json!({
            "authorName": clean_name(&self.author_name),
            "name": self.name(),
            "derivedName": self.derived_name(),
            "allowOutside": self.allow_outside,
            "connected": outside::connected(),
            "trackedChanges": "always",
            "comments": self.comments,
            "folders": self.folders,
            "enabled": self.enabled,
            "provider": p.id(),
            "model": self.model(p),
            "baseUrl": self.base_url(p),
            "hasKey": has_key,
            "usable": usable,
        })
    }
}

/// Assistants connected through the control channel right now.
pub mod outside {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    type Closer = Box<dyn Fn() + Send>;

    static CONNECTIONS: Mutex<Vec<(u64, Closer)>> = Mutex::new(Vec::new());
    static NEXT: AtomicU64 = AtomicU64::new(1);

    fn with<R>(f: impl FnOnce(&mut Vec<(u64, Closer)>) -> R) -> R {
        let mut g = CONNECTIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut g)
    }

    /// A connection that counts as connected until this is dropped. `close` ends the connection
    /// (it is called by [`disconnect_all`]).
    pub struct Registration(u64);

    pub fn register(close: impl Fn() + Send + 'static) -> Registration {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        with(|v| v.push((id, Box::new(close))));
        Registration(id)
    }

    impl Drop for Registration {
        fn drop(&mut self) {
            let id = self.0;
            with(|v| v.retain(|(i, _)| *i != id));
        }
    }

    /// How many assistants are connected.
    pub fn connected() -> usize {
        with(|v| v.len())
    }

    /// Drop every connection. A client may connect again (unless the setting forbids it).
    pub fn disconnect_all() {
        with(|v| {
            for (_, close) in v.iter() {
                close();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{ControlRequest, Outcome, handle_outside};
    use crate::{Services, WordApp};
    use wordcraft_engine::Session;

    fn app() -> WordApp {
        let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
        a.session.author = "Martijn".into();
        a
    }

    /// One request the way a control-port connection sends it.
    fn outside(a: &mut WordApp, method: &str, params: Value) -> Value {
        let ctx = egui::Context::default();
        let (req, _rx) = ControlRequest::new_outside(method, params);
        match handle_outside(a, &ctx, &req) {
            Outcome::Done(v) => v,
            Outcome::Screenshot { .. } => json!({"ok": true, "screenshot": true}),
        }
    }

    fn exec(a: &mut WordApp, command: &str, params: Value) -> Value {
        outside(a, "engine.execute", json!({"command": command, "params": params}))
    }

    fn change_authors(a: &mut WordApp) -> Vec<String> {
        let v = a.session.run("review.changes", &json!({})).unwrap_or_default();
        v.as_array().map(|l| l.iter().filter_map(|c| c["author"].as_str().map(str::to_string)).collect()).unwrap_or_default()
    }

    #[test]
    fn an_edit_over_the_control_channel_is_a_tracked_change_by_the_assistant() {
        let mut a = app();
        a.session.run("text.insert", &json!({"text": "Hello"})).unwrap();
        let r = exec(&mut a, "text.insert", json!({"text": " there"}));
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(change_authors(&mut a), vec!["AI · Mistral".to_string()]);
        // The person's own state is as it was.
        assert!(!a.session.doc.settings.track_changes);
        assert_eq!(a.session.author, "Martijn");
        // A command sent as a bare method name goes the same way.
        outside(&mut a, "text.insert", json!({"text": "!"}));
        assert!(change_authors(&mut a).iter().all(|n| n == "AI · Mistral"));
        assert!(!a.session.doc.settings.track_changes);
    }

    #[test]
    fn accepting_is_refused_with_a_clear_error_and_changes_stay() {
        let mut a = app();
        exec(&mut a, "text.insert", json!({"text": "Hello"}));
        for (m, p) in [
            ("engine.execute", json!({"command": "review.acceptAll"})),
            ("engine.execute", json!({"command": "review.rejectAll", "params": {"author": "Claude"}})),
            ("engine.execute", json!({"command": "review.accept"})),
            ("review.acceptAll", json!({})),
            ("command", json!({"id": "review.reject"})),
        ] {
            let r = outside(&mut a, m, p);
            assert_eq!(r["ok"], false);
            assert!(r["error"].as_str().unwrap_or_default().starts_with("not allowed for assistants: review."), "{r}");
        }
        assert_eq!(change_authors(&mut a), vec!["AI · Mistral".to_string()]);
    }

    #[test]
    fn tracking_author_settings_and_raw_input_are_refused() {
        let mut a = app();
        for (m, p) in [
            ("engine.execute", json!({"command": "review.trackChanges", "params": {"value": false}})),
            ("engine.execute", json!({"command": "file.setAuthor", "params": {"name": "Someone"}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {"authorName": "Someone", "allowOutside": false}})),
            ("ui.assistant", json!({"authorName": "Someone"})),
            ("ui.click", json!({"x": 10, "y": 10})),
            ("ui.key", json!({"key": "E", "cmd": true, "shift": true})),
            ("ui.menu", json!({"path": "Review/Accept Change"})),
        ] {
            let r = outside(&mut a, m, p);
            assert_eq!(r["ok"], false, "{m}");
            assert!(r["error"].as_str().unwrap_or_default().starts_with("not allowed for assistants: "), "{r}");
        }
        assert_eq!(a.ui.assistant, AssistantPrefs::default());
        assert_eq!(a.session.author, "Martijn");
        assert!(!a.session.doc.settings.track_changes);
        // Looking stays allowed.
        assert_eq!(outside(&mut a, "ui.inspect", json!({}))["ok"], true);
        assert_eq!(outside(&mut a, "document.inspect", json!({}))["ok"], true);
        assert_eq!(outside(&mut a, "engine.commands", json!({}))["ok"], true);
        assert_eq!(outside(&mut a, "ui.screenshot", json!({}))["screenshot"], true);
        assert_eq!(exec(&mut a, "review.changes", json!({}))["ok"], true);
    }

    #[test]
    fn a_comment_over_the_channel_is_authored_by_the_assistant() {
        let mut a = app();
        a.session.run("text.insert", &json!({"text": "Hello"})).unwrap();
        let r = exec(&mut a, "review.newComment", json!({"text": "Check this"}));
        assert_eq!(r["ok"], true, "{r}");
        let list = a.session.run("review.comments", &json!({})).unwrap();
        assert_eq!(list[0]["author"], "AI · Mistral");
        assert_eq!(a.session.author, "Martijn");
    }

    #[test]
    fn renaming_the_assistant_changes_the_author_of_the_next_outside_edit() {
        let mut a = app();
        exec(&mut a, "text.insert", json!({"text": "one"}));
        let r = a.run("ui.assistant", json!({"authorName": "  Optimo AI \n"})).unwrap();
        assert_eq!(r["authorName"], "Optimo AI");
        assert_eq!(r["name"], "AI · Optimo AI");
        exec(&mut a, "text.insert", json!({"text": "two"}));
        let authors = change_authors(&mut a);
        assert!(authors.contains(&"AI · Mistral".to_string()) && authors.contains(&"AI · Optimo AI".to_string()), "{authors:?}");
        assert_eq!(crate::review_panel::assistant_author_name(), "AI · Optimo AI");
        // Blank goes back to the model's name; a typed prefix is not doubled.
        assert_eq!(a.run("ui.assistant", json!({"authorName": "  "})).unwrap()["name"], "AI · Mistral");
        assert_eq!(a.run("ui.assistant", json!({"authorName": "AI · Claude"})).unwrap()["name"], "AI · Claude");
        assert!(a.ui.assistant.legacy_names().contains(&"Optimo AI".to_string()), "the old typed name stays recognised");
        assert_eq!(a.session.author, "Martijn");
    }

    #[test]
    fn the_settings_survive_ui_json_and_a_bad_value_falls_back() {
        let mut a = app();
        a.run("ui.assistant", json!({"authorName": "Optimo AI", "allowOutside": false})).unwrap();
        let bytes = serde_json::to_vec(&a.ui).unwrap();
        let back = crate::UiState::from_json(&bytes).unwrap();
        assert_eq!(back.assistant, AssistantPrefs { author_name: "Optimo AI".into(), allow_outside: false, ..AssistantPrefs::default() });
        let odd = crate::UiState::from_json(br#"{"assistant": {"authorName": 5, "allowOutside": "yes"}}"#).unwrap();
        assert_eq!(odd.assistant, AssistantPrefs::default());
        assert_eq!(crate::UiState::from_json(b"{}").unwrap().assistant, AssistantPrefs::default());
    }

    #[test]
    fn switching_outside_assistants_off_refuses_them_and_drops_the_connected() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut a = app();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let _conn = outside::register(move || {
            h.fetch_add(1, Ordering::SeqCst);
        });
        a.run("ui.assistant", json!({"allowOutside": false})).unwrap();
        assert!(hits.load(Ordering::SeqCst) >= 1, "Disconnect ran");
        for m in ["engine.execute", "ui.inspect", "ui.screenshot"] {
            let r = outside(&mut a, m, json!({"command": "text.insert", "params": {"text": "x"}}));
            assert_eq!(r["ok"], false, "{m}");
            assert!(r["error"].as_str().unwrap_or_default().contains("not allowed to connect"), "{r}");
        }
        assert!(change_authors(&mut a).is_empty());
        a.run("ui.assistant", json!({"allowOutside": true})).unwrap();
        assert_eq!(exec(&mut a, "text.insert", json!({"text": "x"}))["ok"], true);
    }

    #[test]
    fn names_are_cleaned_prefixed_and_never_blank() {
        let mut p = AssistantPrefs::default();
        assert_eq!(p.name(), "AI · Mistral", "no name, no model: the provider's");
        p.models.insert("mistral".into(), "mistral-large-4".into());
        assert_eq!(p.name(), "AI · Mistral Large 4", "no name: the model's");
        p.author_name = "  Claude (copyedit)\n".into();
        assert_eq!(p.name(), "AI · Claude (copyedit)");
        p.author_name = "AI · Claude".into();
        assert_eq!(p.name(), "AI · Claude", "never doubled");
        p.author_name = " \t ".into();
        assert_eq!(p.name(), "AI · Mistral Large 4");
        p.provider = "other".into();
        assert_eq!(p.name(), "AI · Assistant", "nothing to go by");
        p.author_name = "é".repeat(10_000);
        assert!(p.name().chars().count() <= MAX_AUTHOR_CHARS + 5);
        assert_eq!(with_prefix(""), "AI · Assistant");
    }

    #[test]
    fn model_ids_read_as_names() {
        for (id, want) in [
            ("claude-opus-5-5", Some("Claude Opus 5.5")),
            ("claude-sonnet-4-5-20250929", Some("Claude Sonnet 4.5")),
            ("gpt-4-1", Some("GPT 4.1")),
            ("gpt-4o", Some("GPT 4o")),
            ("mistral-large-4", Some("Mistral Large 4")),
            ("mistral-large-latest", Some("Mistral Large")),
            ("mistral-large-2411", Some("Mistral Large")),
            ("mistralai/mistral-large", Some("Mistral Large")),
            ("gemini-2.5-pro", Some("Gemini 2.5 Pro")),
            ("llama3.1:8b", Some("Llama 3.1 8B")),
            ("qwen2.5:14b-instruct", Some("Qwen 2.5 14B Instruct")),
            ("deepseek-r1", Some("Deepseek R1")),
            ("glm-4-6", Some("GLM 4.6")),
            ("o3-mini", Some("O3 Mini")),
            ("claude-3-5-haiku-2024-10-22", Some("Claude 3.5 Haiku")),
            ("", None),
            ("latest", None),
            ("///", None),
        ] {
            assert_eq!(model_display_name(id).as_deref(), want, "{id}");
        }
        assert!(model_display_name(&"x-".repeat(500)).is_some_and(|n| n.chars().count() <= MAX_MODEL_NAME_CHARS));
        for junk in ["\u{0}", "-_-:", "\u{1F469}\u{200D}\u{1F467}-1-2", "1-2-3", "2025-01-01", "a/b/c:d:e"] {
            let _ = model_display_name(junk);
        }
    }

    #[test]
    fn a_registration_counts_until_dropped_and_disconnect_calls_the_closer() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let r = outside::register(move || {
            h.fetch_add(1, Ordering::SeqCst);
        });
        assert!(outside::connected() >= 1);
        outside::disconnect_all();
        assert!(hits.load(Ordering::SeqCst) >= 1);
        drop(r);
    }
}
