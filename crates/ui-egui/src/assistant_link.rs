//! Settings › Co-writer S1–S3 at work: the master switch, the provider and its model list, and
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
    /// Keychain lookups still running: the keychain may block on its prompt, so it is never
    /// asked on the UI thread.
    lookups: HashMap<ProviderId, Receiver<Option<String>>>,
    /// Wakes the window when a lookup answers (set by [`poll`]).
    wake: Option<egui::Context>,
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
            lookups: HashMap::new(),
            wake: None,
            models: HashMap::new(),
            job: None,
            last: None,
            key_draft: String::new(),
            changing_key: false,
            key_error: None,
        }
    }

    /// The key store (read off the UI thread by the calls that need a key).
    pub(crate) fn key_store(&self) -> Arc<dyn KeyStore> {
        self.keys.clone()
    }

    /// Whether keys can be kept on this system at all.
    pub fn keys_supported(&self) -> bool {
        self.keys.supported()
    }

    /// `••••abcd` for the stored key of `p`, if there is one. The keychain is asked once, on a
    /// background thread; `None` until it answers (see [`AssistantLink::looking_up`]).
    pub fn hint(&mut self, p: ProviderId) -> Option<String> {
        if let Some(h) = self.hints.get(&p) {
            return h.clone();
        }
        if let Some(rx) = self.lookups.get(&p) {
            let h = match rx.try_recv() {
                Ok(h) => h,
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            };
            self.lookups.remove(&p);
            self.hints.insert(p, h.clone());
            return h;
        }
        self.look_up(p);
        None
    }

    /// The keychain hasn't answered for `p` yet.
    pub fn looking_up(&self, p: ProviderId) -> bool {
        self.lookups.contains_key(&p)
    }

    fn look_up(&mut self, p: ProviderId) {
        let (tx, rx) = std::sync::mpsc::channel();
        let keys = self.keys.clone();
        let wake = self.wake.clone();
        let read = move || {
            let h = match keys.get(p.id()) {
                Ok(k) => k.map(|k| wordcraft_assistant::masked(&k)),
                Err(e) => {
                    log::warn!("assistant: reading the {} key: {e}", p.id());
                    None
                }
            };
            let _ = tx.send(h);
            if let Some(c) = &wake {
                c.request_repaint();
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        if let Err(e) = std::thread::Builder::new().name("assistant-key".into()).spawn(read) {
            log::warn!("assistant: could not read the {} key: {e}", p.id());
            self.hints.insert(p, None);
            return;
        }
        // ponytail: wasm has no threads and no keychain (memory keys), so it reads inline.
        #[cfg(target_arch = "wasm32")]
        read();
        self.lookups.insert(p, rx);
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
            return Err(AssistantError::Unsupported("this system has no keychain Optimo can use, so the key can't be stored".into()).to_string());
        }
        self.keys.set(p.id(), &key).map_err(|e| e.to_string())?;
        self.lookups.remove(&p);
        self.hints.insert(p, Some(wordcraft_assistant::masked(&key)));
        self.models.remove(&p);
        if self.last.as_ref().is_some_and(|l| l.provider == p) {
            self.last = None;
        }
        Ok(())
    }

    fn remove_key(&mut self, p: ProviderId) -> Result<(), String> {
        self.keys.remove(p.id()).map_err(|e| e.to_string())?;
        self.lookups.remove(&p);
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
    if app.assistant_link.wake.is_none() {
        app.assistant_link.wake = app.ctx.clone();
    }
    if let Some(job) = &app.assistant_link.job {
        match job.rx.try_recv() {
            Ok(answer) => finish(app, answer),
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => finish(app, Err(AssistantError::Protocol("the connection stopped".into()))),
        }
    }
    let on = app.ui.assistant.enabled && app.assistant_link.usable(&app.ui.assistant);
    crate::review_panel::set_assistant_enabled(on);
    crate::assistant_link::sync_names(&app.ui.assistant);
    sync_policy(app);
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
        return Err("turn on “Let a co-writer work in Optimo” first: nothing is sent anywhere while it is off".into());
    }
    let p = app.ui.assistant.provider_id();
    if app.assistant_link.busy() {
        return Ok(json!({"started": false, "busy": true, "provider": p.id()}));
    }
    let url = app.ui.assistant.base_url(p);
    // The address is checked here, so a mistake shows at once.
    Endpoint::new(p, Some(&url), None).map_err(|e| e.to_string())?;
    if p.preset().needs_key && app.assistant_link.hint(p).is_none() {
        if app.assistant_link.looking_up(p) {
            return Err("waiting for the keychain to hand over the key: answer its prompt, then try again".into());
        }
        return Err(AssistantError::NoKey.to_string());
    }
    spawn(app, p, url)?;
    Ok(json!({"started": true, "provider": p.id()}))
}

#[cfg(target_arch = "wasm32")]
fn spawn(_app: &mut WordApp, _p: ProviderId, _url: String) -> Result<(), String> {
    Err("the assistant works in the Optimo desktop app".into())
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
/// Other), `apiKey` (S3, stored in the keychain for the chosen provider; write-only),
/// `removeKey: true`, and the permissions of S5: `comments` (add and reply to comments),
/// `addFolder` / `removeFolder` (the folders assistants may read from, read-only) and
/// `pickFolder: true` (the native folder picker, which adds its answer). No value reads them. Replies never contain the key, only `hasKey`.
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
        // Optional: blank goes back to the name derived from the model. The "AI · " prefix is
        // always added, so a typed one is dropped.
        Some(v) => Some(
            v.as_str()
                .map(crate::assistant::clean_name)
                .ok_or("`authorName` must be a string")?
                .trim_start_matches(wordcraft_engine::authorship::AI_PREFIX)
                .trim()
                .to_string(),
        ),
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
    let comments = bool_of("comments")?;
    let pick_folder = bool_of("pickFolder")?;
    let add_folder = match obj.get("addFolder") {
        None => None,
        Some(v) => {
            let f = crate::assistant::check_folder(v.as_str().ok_or("`addFolder` must be a string")?)?;
            if !prefs.folders.contains(&f) && prefs.folders.len() >= crate::assistant::MAX_FOLDERS {
                return Err(format!("at most {} folders can be shared", crate::assistant::MAX_FOLDERS));
            }
            Some(f)
        }
    };
    let remove_folder = match obj.get("removeFolder") {
        None => None,
        Some(v) => {
            let raw = v.as_str().ok_or("`removeFolder` must be a string")?.trim();
            // As it is listed, or as the file system names it now.
            let canon = std::fs::canonicalize(raw).ok().map(|p| p.to_string_lossy().into_owned());
            let found = prefs.folders.iter().find(|f| f.as_str() == raw || Some(f.as_str()) == canon.as_deref()).cloned();
            Some(found.ok_or("that folder isn't in the list")?)
        }
    };
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
        // The old typed name stays recognised (suggestions written under it before the prefix).
        let old = prefs.author_name.clone();
        prefs.remember_name(&old);
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
    if let Some(on) = comments {
        prefs.comments = on;
    }
    if let Some(f) = add_folder
        && !prefs.folders.contains(&f)
    {
        prefs.folders.push(f);
    }
    if let Some(f) = remove_folder {
        prefs.folders.retain(|x| *x != f);
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
    crate::assistant_link::sync_names(&app.ui.assistant);
    sync_policy(app);
    if pick_folder == Some(true) {
        pick_folder_dialog(app);
    }
    Ok(settings_json(app))
}

/// Copy the effective author name ("AI · …") and the legacy names where the UI's one rule
/// (`review_panel::is_assistant`) reads them.
pub fn sync_names(prefs: &crate::assistant::AssistantPrefs) {
    crate::review_panel::set_assistant_name(&prefs.name());
    crate::review_panel::set_legacy_names(prefs.legacy_names());
}

/// Hand the session what the permissions say about comments (the engine enforces it), the
/// names assistants write under (the engine's one matcher, `Session::is_assistant_author`,
/// decides by them) and the model id recorded on accepted text.
pub fn sync_policy(app: &mut WordApp) {
    app.session.assistant_comments = app.ui.assistant.comments;
    for n in crate::review_panel::assistant_names() {
        app.session.add_assistant_author(&n);
    }
    let model = app.ui.assistant.model(app.ui.assistant.provider_id()).unwrap_or("");
    if app.session.assistant_model != model {
        app.session.assistant_model = model.to_string();
    }
}

/// The "+" of Read files in these folders: the native folder picker; its answer is added to the
/// list when it arrives.
fn pick_folder_dialog(app: &mut WordApp) {
    let _ = app.ask_file(crate::file_dialogs::FileDialogRequest::Folder, crate::file_dialogs::AfterPick::AssistantFolder);
}

/// The settings as `ui.assistant` reports them.
pub fn settings_json(app: &mut WordApp) -> Value {
    let p = app.ui.assistant.provider_id();
    let has_key = app.assistant_link.hint(p).is_some();
    let usable = app.assistant_link.usable(&app.ui.assistant);
    app.ui.assistant.to_json(has_key, usable)
}

/// Settings › Co-writer: store what is in the key field (the "Save" button and Enter).
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

    /// Wait for the background keychain lookup of `p` to answer.
    fn settle_keys(a: &mut WordApp, p: ProviderId) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.assistant_link.hint(p).is_none() && a.assistant_link.looking_up(p) && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(5));
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
        assert!(e.contains("Let a co-writer work in Optimo"), "{e}");
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
        settle_keys(&mut a, ProviderId::Anthropic);
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
            for p in [
                json!(null),
                json!(1),
                json!("x"),
                json!({"refresh": "x"}),
                json!({"apiKey": null}),
                json!({"baseUrl": "é".repeat(5000)}),
                json!({"addFolder": "é".repeat(5000)}),
                json!({"addFolder": "\0"}),
                json!({"addFolder": "../../.."}),
                json!({"removeFolder": "\0"}),
                json!({"removeFolder": ["a"]}),
                json!({"comments": 1, "folders": 5}),
                json!({"pickFolder": null}),
            ] {
                let _ = a.run(id, p);
            }
        }
    }

    /// A fresh scratch folder holding `shared/a.txt`, `shared/sub/b.md`, `other/c.txt` and `outside/secret.txt`.
    fn scratch(name: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("margin-folders-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["shared/sub", "other", "outside"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        std::fs::write(base.join("shared/a.txt"), "The tower is 330 metres tall.").unwrap();
        std::fs::write(base.join("shared/sub/b.md"), "Paris is in France.").unwrap();
        std::fs::write(base.join("other/c.txt"), "Other file.").unwrap();
        std::fs::write(base.join("outside/secret.txt"), "Secret.").unwrap();
        std::fs::canonicalize(&base).unwrap()
    }

    fn s(p: &std::path::Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn the_folder_list_and_the_comments_switch_are_settings_that_persist() {
        let base = scratch("persist");
        let mut a = app();
        let r = a.run("ui.assistant", json!({})).unwrap();
        assert_eq!(r["comments"], true, "comments are on by default");
        assert_eq!(r["folders"], json!([]), "no folder: no file access");
        let r = a.run("ui.assistant", json!({"addFolder": s(&base.join("shared"))})).unwrap();
        assert_eq!(r["folders"], json!([s(&base.join("shared"))]));
        // Another route to the same folder is the same entry; a second folder joins the list.
        a.run("ui.assistant", json!({"addFolder": format!("{}/sub/..", s(&base.join("shared")))})).unwrap();
        a.run("ui.assistant", json!({"addFolder": s(&base.join("other")), "comments": false})).unwrap();
        assert_eq!(a.ui.assistant.folders.len(), 2);
        assert!(!a.ui.assistant.comments && !a.session.assistant_comments, "the engine is told");
        // Saved in ui.json and back.
        let back = crate::UiState::from_json(&serde_json::to_vec(&a.ui).unwrap()).unwrap().assistant;
        assert_eq!(back.folders, a.ui.assistant.folders);
        assert!(!back.comments);
        // A list that came back damaged falls back to the defaults, not to a crash.
        let odd = crate::UiState::from_json(br#"{"assistant": {"folders": 5, "comments": "no"}}"#).unwrap().assistant;
        assert!(odd.folders.is_empty() && odd.comments);
        // Remove: as listed, and by another route to it.
        a.run("ui.assistant", json!({"removeFolder": s(&base.join("other"))})).unwrap();
        a.run("ui.assistant", json!({"removeFolder": format!("{}/../shared", s(&base.join("other")))})).unwrap();
        assert!(a.ui.assistant.folders.is_empty());
        assert!(a.run("ui.assistant", json!({"removeFolder": s(&base.join("shared"))})).is_err(), "not in the list");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn the_folder_list_refuses_what_is_not_a_folder() {
        let base = scratch("refuse");
        let mut a = app();
        for junk in [
            json!({"addFolder": s(&base.join("shared/a.txt"))}),
            json!({"addFolder": s(&base.join("nope"))}),
            json!({"addFolder": "/"}),
            json!({"addFolder": ""}),
            json!({"addFolder": "a\nb"}),
            json!({"addFolder": 5}),
            json!({"addFolder": null}),
            json!({"removeFolder": 5}),
            json!({"comments": "no"}),
            json!({"pickFolder": "yes"}),
        ] {
            assert!(a.run("ui.assistant", junk.clone()).is_err(), "{junk}");
        }
        assert!(a.ui.assistant.folders.is_empty());
        // A hand-edited ui.json can't put the top of the file system, a relative path or a flood in force.
        a.ui.assistant.folders = vec!["/".into(), "relative".into(), s(&base.join("shared"))];
        assert_eq!(a.ui.assistant.allowed_folders(), vec![s(&base.join("shared"))]);
        a.ui.assistant.folders = (0..100).map(|i| s(&base.join(format!("f{i}")))).collect();
        assert_eq!(a.ui.assistant.allowed_folders().len(), crate::assistant::MAX_FOLDERS);
        // The limit holds for adding too.
        a.ui.assistant.folders = (0..crate::assistant::MAX_FOLDERS).map(|i| s(&base.join(format!("f{i}")))).collect();
        assert!(a.run("ui.assistant", json!({"addFolder": s(&base.join("shared"))})).unwrap_err().contains("at most"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn outside_assistants_cannot_change_the_folders_or_the_comments_switch() {
        let base = scratch("deny");
        let mut a = app();
        a.run("ui.assistant", json!({"addFolder": s(&base.join("shared"))})).unwrap();
        let before = a.ui.assistant.clone();
        for (m, p) in [
            ("engine.execute", json!({"command": "ui.assistant", "params": {"addFolder": s(&base.join("outside"))}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {"removeFolder": s(&base.join("shared"))}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {"comments": false}})),
            ("engine.execute", json!({"command": "ui.assistant", "params": {"pickFolder": true}})),
            ("ui.assistant", json!({"addFolder": "/etc"})),
            ("ui.assistant", json!({"comments": false})),
            ("command", json!({"id": "ui.assistant", "params": {"addFolder": s(&base.join("outside"))}})),
            ("engine.execute", json!({"command": "ui.assistant.facts", "params": {}})),
            ("engine.execute", json!({"command": "ui.assistant.ask", "params": {"instruction": "x", "file": s(&base.join("shared/a.txt"))}})),
        ] {
            let r = outside(&mut a, m, p);
            assert_eq!(r["ok"], false, "{m}: {r}");
            assert!(r["error"].as_str().unwrap_or_default().starts_with("not allowed for assistants: "), "{r}");
        }
        assert_eq!(a.ui.assistant, before);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn outside_assistants_read_files_only_from_the_allowed_folders() {
        let base = scratch("outside-read");
        let mut a = app();
        let open =
            |a: &mut WordApp, p: &std::path::Path| outside(a, "engine.execute", json!({"command": "insert.textFromFile", "params": {"path": s(p)}}));
        // The default: no folder, no file.
        let r = open(&mut a, &base.join("shared/a.txt"));
        assert_eq!(r["ok"], false, "{r}");
        assert!(r["error"].as_str().unwrap().contains("read authority is absent"), "{r}");
        assert!(r["error"].as_str().unwrap().contains("Settings"), "says where to allow folders: {r}");
        let r = outside(&mut a, "engine.execute", json!({"command": "file.open", "params": {"path": s(&base.join("shared/a.txt"))}}));
        assert_eq!(r["ok"], false, "{r}");
        // One folder allowed: files inside (also below it) are readable, others are not.
        a.run("ui.assistant", json!({"addFolder": s(&base.join("shared"))})).unwrap();
        let r = open(&mut a, &base.join("shared/a.txt"));
        assert_eq!(r["ok"], true, "{r}");
        assert!(a.session.doc.plain_text(wordcraft_doc::StoryRef::Body).contains("330 metres"));
        assert_eq!(open(&mut a, &base.join("shared/sub/b.md"))["ok"], true);
        for p in [
            base.join("other/c.txt"),
            base.join("outside/secret.txt"),
            std::path::PathBuf::from(format!("{}/../outside/secret.txt", s(&base.join("shared")))),
        ] {
            let r = open(&mut a, &p);
            assert_eq!(r["ok"], false, "{p:?}: {r}");
            assert!(r["error"].as_str().unwrap().contains("outside the read root"), "{r}");
        }
        // Several folders: any of them.
        a.run("ui.assistant", json!({"addFolder": s(&base.join("other"))})).unwrap();
        assert_eq!(open(&mut a, &base.join("other/c.txt"))["ok"], true);
        // A link inside an allowed folder that leads out is outside.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(base.join("outside/secret.txt"), base.join("shared/leak.txt")).unwrap();
            let r = open(&mut a, &base.join("shared/leak.txt"));
            assert_eq!(r["ok"], false, "{r}");
        }
        // The person's own commands are never held to the list.
        let r = inside(&mut a, "engine.execute", json!({"command": "insert.textFromFile", "params": {"path": s(&base.join("outside/secret.txt"))}}));
        assert_eq!(r["ok"], true, "{r}");
        // Writes are as they were: the folder list says nothing about them.
        let out = base.join("outside/saved.txt");
        let r = outside(&mut a, "engine.execute", json!({"command": "file.saveAs", "params": {"path": s(&out)}}));
        assert_eq!(r["ok"], true, "{r}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn with_comments_off_outside_assistants_are_refused_new_comments_and_replies() {
        let mut a = app();
        a.session.run("text.insert", &json!({"text": "Hello world"})).unwrap();
        let r = outside(&mut a, "engine.execute", json!({"command": "review.newComment", "params": {"text": "ok while on"}}));
        assert_eq!(r["ok"], true, "{r}");
        let id = a.session.run("review.comments", &json!({})).unwrap()[0]["id"].clone();
        a.run("ui.assistant", json!({"comments": false})).unwrap();
        for (m, p) in [
            ("engine.execute", json!({"command": "review.newComment", "params": {"text": "x"}})),
            ("review.newComment", json!({"text": "x"})),
            ("engine.execute", json!({"command": "review.reply", "params": {"id": id, "text": "x"}})),
            ("review.reply", json!({"id": id, "text": "x"})),
        ] {
            let r = outside(&mut a, m, p);
            assert_eq!(r["ok"], false, "{m}: {r}");
            assert!(r["error"].as_str().unwrap_or_default().starts_with("not allowed for assistants: "), "{r}");
        }
        assert_eq!(a.session.doc.comments.len(), 1);
        // The person still can, and tracked text edits by assistants are not affected.
        inside(&mut a, "engine.execute", json!({"command": "review.newComment", "params": {"text": "mine"}}));
        assert_eq!(a.session.doc.comments.len(), 2);
        assert_eq!(outside(&mut a, "engine.execute", json!({"command": "text.insert", "params": {"text": "!"}}))["ok"], true);
        a.run("ui.assistant", json!({"comments": true})).unwrap();
        assert_eq!(outside(&mut a, "engine.execute", json!({"command": "review.newComment", "params": {"text": "again"}}))["ok"], true);
    }
    /// A keychain that blocks like one showing its prompt, until the test answers it.
    struct PromptingKeys(std::sync::Mutex<Receiver<()>>);
    impl KeyStore for PromptingKeys {
        fn supported(&self) -> bool {
            true
        }
        fn get(&self, _account: &str) -> Result<Option<String>, AssistantError> {
            if let Ok(rx) = self.0.lock() {
                let _ = rx.recv_timeout(std::time::Duration::from_secs(20));
            }
            Ok(Some(KEY.into()))
        }
        fn set(&self, _account: &str, _key: &str) -> Result<(), AssistantError> {
            Ok(())
        }
        fn remove(&self, _account: &str) -> Result<(), AssistantError> {
            Ok(())
        }
    }

    /// The window keeps drawing and answering while the keychain prompt is open (2026-10-19:
    /// the main thread hung in SecKeychainFindGenericPassword at startup). Were the keychain read
    /// on the UI thread, this would sit out the mock's 20 s.
    #[test]
    fn a_prompting_keychain_never_blocks_the_window() {
        let (answer, rx) = std::sync::mpsc::channel();
        let services = Services { key_store: Some(Arc::new(PromptingKeys(std::sync::Mutex::new(rx)))), ..Default::default() };
        let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), services);
        a.ui.assistant.enabled = true;
        let t = std::time::Instant::now();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        let a = h.state_mut();
        assert_eq!(settings_json(a)["hasKey"], false, "not known yet");
        assert!(a.assistant_link.looking_up(ProviderId::Mistral));
        assert!(a.run("ui.assistant.test", json!({})).unwrap_err().contains("keychain"));
        assert!(crate::cowriter::ask(a, "hi", wordcraft_engine::assistant_edits::ScopeKind::Document).unwrap_err().contains("keychain"));
        assert!(t.elapsed() < std::time::Duration::from_secs(15), "the window waited for the keychain: {:?}", t.elapsed());
        answer.send(()).unwrap();
        settle_keys(a, ProviderId::Mistral);
        assert_eq!(a.assistant_link.hint(ProviderId::Mistral).as_deref(), Some("••••abcd"));
        assert_eq!(settings_json(a)["hasKey"], true);
    }
}
