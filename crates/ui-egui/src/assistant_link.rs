//! Settings › Assistant S1–S3 at work: the master switch, the provider and its model list, and
//! the API key (HANDOFF §9).
//!
//! * Keys live in the system keychain ([`Services::key_store`](crate::Services::key_store); in
//!   memory when the host gives none) under the provider id. This module keeps only the masked
//!   form (`••••abcd`) for display. No command returns a key, and none is in `ui.json`,
//!   `ui.inspect`, logs or errors.
//! * Nothing is sent while the switch is off. Calls (listing models, which is also "Test
//!   connection") run on a background thread; [`poll`] collects the answer each frame.
//! * Commands: `ui.assistant` (settings, see [`settings_command`]), `ui.assistant.test` and
//!   `ui.assistant.models`. All three are the person's: outside assistants are refused them
//!   (`wordcraft_engine::assistant::denied`).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use serde_json::{Value, json};
use wordcraft_assistant::keys::{KeyStore, MemoryKeys};
use wordcraft_assistant::{AssistantError, Endpoint, Model, ProviderId};

use crate::WordApp;

/// Longest model id accepted.
pub const MAX_MODEL_CHARS: usize = 256;

type Answer = Result<Vec<Model>, AssistantError>;

/// A model listing in flight.
struct Job {
    provider: ProviderId,
    rx: Receiver<Answer>,
}

/// How the last listing (or connection test) ended.
#[derive(Clone, Debug, PartialEq)]
pub struct LastTest {
    pub provider: ProviderId,
    /// Number of models, or the error in plain words.
    pub result: Result<usize, String>,
}

/// The app's assistant connection state (not saved: settings are in
/// [`crate::assistant::AssistantPrefs`], keys in the keychain).
pub struct AssistantLink {
    keys: Arc<dyn KeyStore>,
    /// Masked keys per provider, looked up once (`None` inside: no key stored).
    hints: HashMap<ProviderId, Option<String>>,
    /// Models listed per provider (filled live; "Refresh" lists again).
    pub models: HashMap<ProviderId, Vec<Model>>,
    job: Option<Job>,
    pub last: Option<LastTest>,
    /// The key field while typing or pasting (cleared once stored).
    pub key_draft: String,
    /// "Change…" was clicked: show the field even though a key is stored.
    pub changing_key: bool,
    /// Why the last key couldn't be stored.
    pub key_error: Option<String>,
}

impl Default for AssistantLink {
    fn default() -> Self {
        AssistantLink::new(Arc::new(MemoryKeys::default()))
    }
}

impl AssistantLink {
    pub fn new(keys: Arc<dyn KeyStore>) -> Self {
        AssistantLink {
            keys,
            hints: HashMap::new(),
            models: HashMap::new(),
            job: None,
            last: None,
            key_draft: String::new(),
            changing_key: false,
            key_error: None,
        }
    }

    /// Whether keys can be kept on this system at all.
    pub fn keys_supported(&self) -> bool {
        self.keys.supported()
    }

    /// `••••abcd` for the stored key of `p`, if there is one (the keychain is asked once).
    pub fn hint(&mut self, p: ProviderId) -> Option<String> {
        if let Some(h) = self.hints.get(&p) {
            return h.clone();
        }
        let h = match self.keys.get(p.id()) {
            Ok(k) => k.map(|k| wordcraft_assistant::masked(&k)),
            Err(e) => {
                log::warn!("assistant: reading the {} key: {e}", p.id());
                None
            }
        };
        self.hints.insert(p, h.clone());
        h
    }

    /// Whether a call to the chosen provider could be made (a key where one is needed, an
    /// address for Other). Says nothing about the switch.
    pub fn usable(&mut self, prefs: &crate::assistant::AssistantPrefs) -> bool {
        let p = prefs.provider_id();
        if p.preset().needs_key && self.hint(p).is_none() {
            return false;
        }
        Endpoint::new(p, Some(&prefs.base_url(p)), None).is_ok()
    }

    /// A listing is running.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn store_key(&mut self, p: ProviderId, raw: &str) -> Result<(), String> {
        let key = wordcraft_assistant::clean_key(raw).map_err(|e| e.to_string())?;
        if !self.keys.supported() {
            return Err(AssistantError::Unsupported("this system has no keychain Margin can use, so the key can't be stored".into()).to_string());
        }
        self.keys.set(p.id(), &key).map_err(|e| e.to_string())?;
        self.hints.insert(p, Some(wordcraft_assistant::masked(&key)));
        self.models.remove(&p);
        if self.last.as_ref().is_some_and(|l| l.provider == p) {
            self.last = None;
        }
        Ok(())
    }

    fn remove_key(&mut self, p: ProviderId) -> Result<(), String> {
        self.keys.remove(p.id()).map_err(|e| e.to_string())?;
        self.hints.insert(p, None);
        self.models.remove(&p);
        if self.last.as_ref().is_some_and(|l| l.provider == p) {
            self.last = None;
        }
        Ok(())
    }
}

/// Copy whether an assistant can work where the command field and menus read it, and collect a
/// finished listing. Every frame.
pub fn poll(app: &mut WordApp) {
    if let Some(job) = &app.assistant_link.job {
        match job.rx.try_recv() {
            Ok(answer) => finish(app, answer),
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => finish(app, Err(AssistantError::Protocol("the connection stopped".into()))),
        }
    }
    let on = app.ui.assistant.enabled && app.assistant_link.usable(&app.ui.assistant);
    crate::review_panel::set_assistant_enabled(on);
}

fn finish(app: &mut WordApp, answer: Answer) {
    let Some(job) = app.assistant_link.job.take() else { return };
    let p = job.provider;
    let result = match answer {
        Ok(models) => {
            let n = models.len();
            app.assistant_link.models.insert(p, models);
            log::info!("assistant: {} lists {n} models", p.id());
            Ok(n)
        }
        Err(e) => {
            // The error never holds the key (wordcraft_assistant::AssistantError).
            log::warn!("assistant: {}: {e}", p.id());
            Err(e.to_string())
        }
    };
    app.assistant_link.last = Some(LastTest { provider: p, result });
}

/// Start listing the chosen provider's models (also "Test connection").
pub fn start(app: &mut WordApp) -> Result<Value, String> {
    if !app.ui.assistant.enabled {
        return Err("turn on “Let an assistant work in Margin” first: nothing is sent anywhere while it is off".into());
    }
    let p = app.ui.assistant.provider_id();
    if app.assistant_link.busy() {
        return Ok(json!({"started": false, "busy": true, "provider": p.id()}));
    }
    let url = app.ui.assistant.base_url(p);
    // The address is checked here, so a mistake shows at once.
    Endpoint::new(p, Some(&url), None).map_err(|e| e.to_string())?;
    if p.preset().needs_key && app.assistant_link.hint(p).is_none() {
        return Err(AssistantError::NoKey.to_string());
    }
    spawn(app, p, url)?;
    Ok(json!({"started": true, "provider": p.id()}))
}

#[cfg(target_arch = "wasm32")]
fn spawn(_app: &mut WordApp, _p: ProviderId, _url: String) -> Result<(), String> {
    Err("the assistant works in the Margin desktop app".into())
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn(app: &mut WordApp, p: ProviderId, url: String) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let keys = app.assistant_link.keys.clone();
    let wake = app.ctx.clone();
    let spawned = std::thread::Builder::new().name("assistant".into()).spawn(move || {
        // The key is read here, off the UI thread (the keychain may take a moment).
        let answer = keys
            .get(p.id())
            .and_then(|key| Endpoint::new(p, Some(&url), key))
            .and_then(|ep| wordcraft_assistant::http::list_models(&ep, wordcraft_assistant::DEFAULT_TIMEOUT));
        let _ = tx.send(answer);
        if let Some(c) = &wake {
            c.request_repaint();
        }
    });
    if let Err(e) = spawned {
        return Err(format!("could not start the connection: {e}"));
    }
    app.assistant_link.job = Some(Job { provider: p, rx });
    app.assistant_link.last = None;
    Ok(())
}

fn status(app: &mut WordApp) -> Value {
    let p = app.ui.assistant.provider_id();
    let link = &app.assistant_link;
    let models: Vec<Value> = link.models.get(&p).map(|m| m.iter().map(|m| json!({"id": m.id, "name": m.name})).collect()).unwrap_or_default();
    let last = link.last.as_ref().map(|l| match &l.result {
        Ok(n) => json!({"provider": l.provider.id(), "ok": true, "models": n}),
        Err(e) => json!({"provider": l.provider.id(), "ok": false, "error": e}),
    });
    json!({"provider": p.id(), "models": models, "busy": link.busy(), "last": last, "model": app.ui.assistant.model(p)})
}

/// `ui.assistant.test` / `ui.assistant.models`; `None` for other ids.
///
/// * `ui.assistant.test`: list the chosen provider's models in the background (the "Test
///   connection" button); the outcome shows in Settings and in `ui.assistant.models`' `last`.
/// * `ui.assistant.models`: the models listed for the chosen provider, whether a listing is
///   running, and how the last one ended; `{"refresh": true}` lists again first.
pub fn command(app: &mut WordApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    match id {
        "ui.assistant.test" => Some(start(app)),
        "ui.assistant.models" => {
            if params.get("refresh").and_then(Value::as_bool) == Some(true)
                && let Err(e) = start(app)
            {
                return Some(Err(e));
            }
            Some(Ok(status(app)))
        }
        _ => None,
    }
}

/// `ui.assistant`: change any of `authorName` (S4), `allowOutside` (S6), `enabled` (S1),
/// `provider` (S2, a provider id), `model` (for the chosen provider), `baseUrl` (On this Mac and
/// Other), `apiKey` (S3, stored in the keychain for the chosen provider; write-only) and
/// `removeKey: true`. No value reads them. Replies never contain the key, only `hasKey`.
pub fn settings_command(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    let none = serde_json::Map::new();
    let obj = match p {
        Value::Null => &none,
        Value::Object(o) => o,
        _ => return Err("give an object of settings".into()),
    };
    let prefs = &mut app.ui.assistant;
    // Check everything before changing anything.
    let name = match obj.get("authorName") {
        None => None,
        Some(v) => Some(v.as_str().map(crate::assistant::clean_name).filter(|n| !n.is_empty()).ok_or("`authorName` must be a non-blank string")?),
    };
    let bool_of = |k: &str| -> Result<Option<bool>, String> {
        match obj.get(k) {
            None => Ok(None),
            Some(v) => v.as_bool().map(Some).ok_or_else(|| format!("`{k}` must be true or false")),
        }
    };
    let allow = bool_of("allowOutside")?;
    let enabled = bool_of("enabled")?;
    let remove_key = bool_of("removeKey")?;
    let provider = match obj.get("provider") {
        None => None,
        Some(v) => {
            let ids: Vec<&str> = ProviderId::ALL.iter().map(|p| p.id()).collect();
            Some(v.as_str().and_then(ProviderId::from_id).ok_or_else(|| format!("`provider` must be one of {}", ids.join(", ")))?)
        }
    };
    let chosen = provider.unwrap_or(prefs.provider_id());
    let model = match obj.get("model") {
        None => None,
        Some(v) => {
            let m = v.as_str().map(str::trim).ok_or("`model` must be a string")?;
            if m.chars().count() > MAX_MODEL_CHARS || m.chars().any(char::is_control) {
                return Err("`model` isn't a model id".into());
            }
            Some(m.to_string())
        }
    };
    let base_url = match obj.get("baseUrl") {
        None => None,
        Some(v) => {
            let u = v.as_str().ok_or("`baseUrl` must be a string")?.trim();
            if !chosen.preset().editable_url {
                return Err(format!("{} has a fixed address", chosen.label()));
            }
            if u.is_empty() { Some(String::new()) } else { Some(wordcraft_assistant::check_url(u).map_err(|e| e.to_string())?) }
        }
    };
    // The key: checked, never echoed.
    let key = match obj.get("apiKey") {
        None => None,
        Some(v) => Some(v.as_str().ok_or("`apiKey` must be a string")?.to_string()),
    };
    if let Some(n) = name {
        prefs.author_name = n;
    }
    if let Some(on) = allow {
        prefs.allow_outside = on;
        if !on {
            crate::assistant::outside::disconnect_all();
        }
    }
    if let Some(on) = enabled {
        prefs.enabled = on;
    }
    if let Some(p) = provider {
        prefs.provider = p.id().to_string();
    }
    if let Some(m) = model {
        prefs.models.insert(chosen.id().to_string(), m);
    }
    if let Some(u) = base_url {
        prefs.base_urls.insert(chosen.id().to_string(), u);
        app.assistant_link.models.remove(&chosen);
    }
    if remove_key == Some(true) {
        app.assistant_link.remove_key(chosen)?;
    }
    if let Some(k) = key {
        app.assistant_link.store_key(chosen, &k)?;
        app.assistant_link.key_draft.clear();
        app.assistant_link.changing_key = false;
        // A new key lists the models at once, when the switch allows it.
        if app.ui.assistant.enabled
            && let Err(e) = start(app)
        {
            log::info!("assistant: not listing models: {e}");
        }
    }
    crate::review_panel::set_assistant_name(&app.ui.assistant.name());
    Ok(settings_json(app))
}

/// The settings as `ui.assistant` reports them.
pub fn settings_json(app: &mut WordApp) -> Value {
    let p = app.ui.assistant.provider_id();
    let has_key = app.assistant_link.hint(p).is_some();
    let usable = app.assistant_link.usable(&app.ui.assistant);
    app.ui.assistant.to_json(has_key, usable)
}

/// Settings › Assistant: store what is in the key field (the "Save" button and Enter).
pub fn save_draft_key(app: &mut WordApp) {
    let draft = std::mem::take(&mut app.assistant_link.key_draft);
    match settings_command(app, &json!({"apiKey": draft})) {
        Ok(_) => app.assistant_link.key_error = None,
        Err(e) => app.assistant_link.key_error = Some(e),
    }
}

/// Wait for a listing to end (tests).
#[cfg(test)]
pub fn wait(app: &mut WordApp) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while app.assistant_link.busy() && std::time::Instant::now() < end {
        poll(app);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    poll(app);
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;
    use crate::Services;
    use crate::control::{ControlRequest, Outcome, handle_outside};
    use wordcraft_engine::Session;

    const KEY: &str = "sk-margin-test-9f8e7d6c5b4aabcd";

    fn app() -> WordApp {
        WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default())
    }

    fn outside(a: &mut WordApp, method: &str, params: Value) -> Value {
        let ctx = egui::Context::default();
        let (req, _rx) = ControlRequest::new_outside(method, params);
        match handle_outside(a, &ctx, &req) {
            Outcome::Done(v) => v,
            Outcome::Screenshot { .. } => json!({"ok": true}),
        }
    }

    fn inside(a: &mut WordApp, method: &str, params: Value) -> Value {
        let ctx = egui::Context::default();
        let (req, _rx) = ControlRequest::new(method, params);
        match crate::control::handle(a, &ctx, &req) {
            Outcome::Done(v) => v,
            Outcome::Screenshot { .. } => json!({"ok": true}),
        }
    }

    /// A listener that answers one model listing per connection and counts connections.
    fn mock_models() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{BufRead, BufReader, Write};
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}/v1", l.local_addr().unwrap().port());
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let h = hits.clone();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                    line.clear();
                }
                let body = r#"{"data":[{"id":"local-a"},{"id":"local-b"}]}"#;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (base, hits)
    }

    #[test]
    fn the_assistant_ships_off_with_mistral_selected() {
        let mut a = app();
        let p = crate::assistant::AssistantPrefs::default();
        assert!(!p.enabled);
        assert_eq!(p.provider_id(), ProviderId::Mistral);
        poll(&mut a);
        assert!(!crate::review_panel::assistant_enabled());
        let r = a.run("ui.assistant", json!({})).unwrap();
        assert_eq!((r["enabled"].clone(), r["provider"].clone(), r["hasKey"].clone()), (json!(false), json!("mistral"), json!(false)));
    }

    #[test]
    fn enabled_needs_the_switch_and_a_key_or_a_local_server() {
        let mut a = app();
        a.run("ui.assistant", json!({"enabled": true})).unwrap();
        poll(&mut a);
        assert!(!crate::review_panel::assistant_enabled(), "Mistral without a key");
        // (The key is set while off: a key set while on lists Mistral's models over the network.)
        a.run("ui.assistant", json!({"enabled": false})).unwrap();
        a.run("ui.assistant", json!({"apiKey": KEY})).unwrap();
        assert!(!a.assistant_link.busy());
        a.run("ui.assistant", json!({"enabled": true})).unwrap();
        poll(&mut a);
        assert!(crate::review_panel::assistant_enabled());
        a.run("ui.assistant", json!({"enabled": false})).unwrap();
        poll(&mut a);
        assert!(!crate::review_panel::assistant_enabled(), "the switch wins");
        a.run("ui.assistant", json!({"enabled": true, "provider": "local"})).unwrap();
        poll(&mut a);
        assert!(crate::review_panel::assistant_enabled(), "local needs no key");
        a.run("ui.assistant", json!({"provider": "other"})).unwrap();
        poll(&mut a);
        assert!(!crate::review_panel::assistant_enabled(), "Other needs an address");
        a.run("ui.assistant", json!({"baseUrl": "https://eu.example.com/v1"})).unwrap();
        poll(&mut a);
        assert!(crate::review_panel::assistant_enabled());
    }

    #[test]
    fn the_key_is_stored_masked_and_removed() {
        let mut a = app();
        let r = a.run("ui.assistant", json!({"apiKey": format!("  {KEY}\n")})).unwrap();
        assert_eq!(r["hasKey"], true);
        assert!(!r.to_string().contains(KEY));
        assert_eq!(a.assistant_link.hint(ProviderId::Mistral).as_deref(), Some("••••abcd"));
        assert_eq!(a.assistant_link.keys.get("mistral").unwrap().as_deref(), Some(KEY));
        assert_eq!(a.assistant_link.hint(ProviderId::Anthropic), None, "one key per provider");
        assert!(a.run("ui.assistant", json!({"apiKey": "two words"})).is_err());
        assert!(a.run("ui.assistant", json!({"apiKey": 5})).is_err());
        let r = a.run("ui.assistant", json!({"removeKey": true})).unwrap();
        assert_eq!(r["hasKey"], false);
        assert_eq!(a.assistant_link.keys.get("mistral").unwrap(), None);
    }

    #[test]
    fn the_key_is_in_no_saved_setting_and_no_reply() {
        let mut a = app();
        a.run("ui.assistant", json!({"apiKey": KEY, "provider": "anthropic", "model": "claude-x"})).unwrap();
        a.run("ui.assistant", json!({"provider": "mistral", "apiKey": KEY})).unwrap();
        a.assistant_link.key_draft = KEY.into();
        let saved = String::from_utf8(serde_json::to_vec(&a.ui).unwrap()).unwrap();
        assert!(!saved.contains(KEY) && !saved.contains("abcd"), "ui.json");
        let prefs = serde_json::to_string(&a.prefs()).unwrap();
        assert!(!prefs.contains(KEY));
        for (m, p) in [
            ("ui.inspect", json!({})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {}})),
            ("engine.execute", json!({"command": "ui.assistant.models", "params": {}})),
            ("ui.assistant", json!({})),
            ("ui.assistant.models", json!({})),
        ] {
            let r = inside(&mut a, m, p.clone());
            assert!(!r.to_string().contains(KEY), "{m}: {r}");
            let r = outside(&mut a, m, p);
            assert!(!r.to_string().contains(KEY), "outside {m}: {r}");
        }
        assert!(!format!("{:?}", a.ui.assistant).contains(KEY));
    }

    #[test]
    fn outside_assistants_cannot_touch_the_provider_key_or_switch() {
        let mut a = app();
        a.run("ui.assistant", json!({"apiKey": KEY})).unwrap();
        for (m, p) in [
            ("engine.execute", json!({"command": "ui.assistant", "params": {"enabled": true}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {"provider": "other", "baseUrl": "https://evil.example/v1"}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {"removeKey": true}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {}})),
            ("engine.execute", json!({"command": "ui.assistant.test", "params": {}})),
            ("engine.execute", json!({"command": "ui.assistant.models", "params": {}})),
            ("ui.assistant", json!({"apiKey": "sk-other"})),
            ("ui.assistant.test", json!({})),
            ("ui.assistant.models", json!({"refresh": true})),
            ("command", json!({"id": "ui.assistant.models"})),
        ] {
            let r = outside(&mut a, m, p);
            assert_eq!(r["ok"], false, "{m}: {r}");
            assert!(r["error"].as_str().unwrap_or_default().starts_with("not allowed for assistants: "), "{r}");
        }
        assert!(!a.ui.assistant.enabled);
        assert_eq!(a.ui.assistant.provider_id(), ProviderId::Mistral);
        assert_eq!(a.assistant_link.keys.get("mistral").unwrap().as_deref(), Some(KEY));
    }

    #[test]
    fn nothing_is_sent_while_the_switch_is_off() {
        let (base, hits) = mock_models();
        let mut a = app();
        a.run("ui.assistant", json!({"provider": "other", "baseUrl": base, "apiKey": KEY})).unwrap();
        let e = a.run("ui.assistant.test", json!({})).unwrap_err();
        assert!(e.contains("Let an assistant work in Margin"), "{e}");
        assert!(a.run("ui.assistant.models", json!({"refresh": true})).is_err());
        assert!(!a.assistant_link.busy());
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
        // On: the test lists the models off the UI thread.
        a.run("ui.assistant", json!({"enabled": true})).unwrap();
        assert_eq!(a.run("ui.assistant.test", json!({})).unwrap()["started"], true);
        wait(&mut a);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        let r = a.run("ui.assistant.models", json!({})).unwrap();
        assert_eq!(r["models"].as_array().map(Vec::len), Some(2));
        assert_eq!(r["last"]["ok"], true);
        assert_eq!(a.assistant_link.models[&ProviderId::Other][0].id, "local-a");
    }

    #[test]
    fn a_failed_test_says_why_in_plain_words() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut a = app();
        a.run("ui.assistant", json!({"enabled": true, "provider": "local", "baseUrl": format!("http://127.0.0.1:{port}/v1")})).unwrap();
        a.run("ui.assistant.test", json!({})).unwrap();
        wait(&mut a);
        let last = a.assistant_link.last.clone().unwrap();
        assert!(last.result.as_ref().unwrap_err().contains("start Ollama or LM Studio"), "{last:?}");
        // A provider that needs a key says so before sending anything.
        a.run("ui.assistant", json!({"provider": "anthropic"})).unwrap();
        assert_eq!(a.run("ui.assistant.test", json!({})).unwrap_err(), "add an API key first");
    }

    #[test]
    fn settings_survive_ui_json_and_junk_is_refused() {
        let mut a = app();
        a.run("ui.assistant", json!({"enabled": true, "provider": "local", "baseUrl": "http://localhost:1234/v1", "model": "qwen"})).unwrap();
        assert!(!a.assistant_link.busy(), "no key was set: nothing listed");
        let back = crate::UiState::from_json(&serde_json::to_vec(&a.ui).unwrap()).unwrap().assistant;
        assert!(back.enabled);
        assert_eq!(back.provider_id(), ProviderId::Local);
        assert_eq!(back.base_url(ProviderId::Local), "http://localhost:1234/v1");
        assert_eq!(back.model(ProviderId::Local), Some("qwen"));
        assert_eq!(back.base_url(ProviderId::Mistral), "https://api.mistral.ai/v1");
        let odd = crate::UiState::from_json(br#"{"assistant": {"provider": "nonsense"}}"#).unwrap().assistant;
        assert_eq!(odd.provider_id(), ProviderId::Mistral);
        for junk in [
            json!({"enabled": "yes"}),
            json!({"provider": "acme"}),
            json!({"provider": 3}),
            json!({"model": "x".repeat(MAX_MODEL_CHARS + 1)}),
            json!({"model": "a\nb"}),
            json!({"baseUrl": "ftp://x"}),
            json!({"provider": "mistral", "baseUrl": "https://x.example"}),
            json!({"removeKey": 1}),
            json!([1, 2]),
            json!("x"),
        ] {
            assert!(a.run("ui.assistant", junk.clone()).is_err(), "{junk}");
        }
        assert!(a.ui.assistant.enabled, "a refused request changes nothing");
    }

    #[test]
    fn hostile_params_never_panic() {
        let mut a = app();
        for id in ["ui.assistant", "ui.assistant.test", "ui.assistant.models"] {
            for p in [json!(null), json!(1), json!("x"), json!({"refresh": "x"}), json!({"apiKey": null}), json!({"baseUrl": "é".repeat(5000)})] {
                let _ = a.run(id, p);
            }
        }
    }
}
