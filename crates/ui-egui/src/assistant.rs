//! Settings › Assistant (HANDOFF §9): what the person decided about assistants, and the
//! bookkeeping for assistants that connect from outside Margin (Claude Desktop, Claude Code, any
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

/// The name assistants write under until the person changes it.
pub const DEFAULT_NAME: &str = "Claude";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AssistantPrefs {
    /// S4: written into assistants' changes and comments.
    pub author_name: String,
    /// S6: assistants outside Margin may use the control channel (which only exists when the app
    /// was started with `--control`). On by default, as the channel itself is an opt-in.
    pub allow_outside: bool,
    /// S1: "Let an assistant work in Margin". Off by default; while off nothing is sent anywhere.
    pub enabled: bool,
    /// S2: the provider id (`wordcraft_assistant::ProviderId::id`); Mistral until chosen.
    pub provider: String,
    /// The model chosen per provider id.
    pub models: BTreeMap<String, String>,
    /// The base URL per provider id, for those where it can be changed (On this Mac, Other).
    pub base_urls: BTreeMap<String, String>,
    // API keys are not here: they live in the system keychain (`crate::assistant_link`).
}

impl Default for AssistantPrefs {
    fn default() -> Self {
        AssistantPrefs {
            author_name: DEFAULT_NAME.into(),
            allow_outside: true,
            enabled: false,
            provider: ProviderId::Mistral.id().into(),
            models: BTreeMap::new(),
            base_urls: BTreeMap::new(),
        }
    }
}

/// An author name as the engine stores it: trimmed, no control characters, capped.
pub fn clean_name(raw: &str) -> String {
    let n: String = raw.trim().chars().filter(|c| !c.is_control()).take(MAX_AUTHOR_CHARS).collect();
    n.trim_end().to_string()
}

impl AssistantPrefs {
    /// The author name to use: the setting, or the default while it is blank.
    pub fn name(&self) -> String {
        let n = clean_name(&self.author_name);
        if n.is_empty() { DEFAULT_NAME.into() } else { n }
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
            "authorName": self.name(),
            "allowOutside": self.allow_outside,
            "connected": outside::connected(),
            "trackedChanges": "always",
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
        assert_eq!(change_authors(&mut a), vec!["Claude".to_string()]);
        // The person's own state is as it was.
        assert!(!a.session.doc.settings.track_changes);
        assert_eq!(a.session.author, "Martijn");
        // A command sent as a bare method name goes the same way.
        outside(&mut a, "text.insert", json!({"text": "!"}));
        assert!(change_authors(&mut a).iter().all(|n| n == "Claude"));
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
        assert_eq!(change_authors(&mut a), vec!["Claude".to_string()]);
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
        assert_eq!(list[0]["author"], "Claude");
        assert_eq!(a.session.author, "Martijn");
    }

    #[test]
    fn renaming_the_assistant_changes_the_author_of_the_next_outside_edit() {
        let mut a = app();
        exec(&mut a, "text.insert", json!({"text": "one"}));
        let r = a.run("ui.assistant", json!({"authorName": "  Margin AI \n"})).unwrap();
        assert_eq!(r["authorName"], "Margin AI");
        assert!(a.run("ui.assistant", json!({"authorName": "  "})).is_err());
        exec(&mut a, "text.insert", json!({"text": "two"}));
        let authors = change_authors(&mut a);
        assert!(authors.contains(&"Claude".to_string()) && authors.contains(&"Margin AI".to_string()), "{authors:?}");
        assert_eq!(crate::review_panel::assistant_author_name(), "Margin AI");
        assert_eq!(a.session.author, "Martijn");
    }

    #[test]
    fn the_settings_survive_ui_json_and_a_bad_value_falls_back() {
        let mut a = app();
        a.run("ui.assistant", json!({"authorName": "Margin AI", "allowOutside": false})).unwrap();
        let bytes = serde_json::to_vec(&a.ui).unwrap();
        let back = crate::UiState::from_json(&bytes).unwrap();
        assert_eq!(back.assistant, AssistantPrefs { author_name: "Margin AI".into(), allow_outside: false, ..AssistantPrefs::default() });
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
    fn names_are_cleaned_and_never_blank() {
        let mut p = AssistantPrefs::default();
        assert_eq!(p.name(), "Claude");
        p.author_name = "  Claude (copyedit)\n".into();
        assert_eq!(p.name(), "Claude (copyedit)");
        p.author_name = " \t ".into();
        assert_eq!(p.name(), "Claude");
        p.author_name = "é".repeat(10_000);
        assert_eq!(p.name().chars().count(), MAX_AUTHOR_CHARS);
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
