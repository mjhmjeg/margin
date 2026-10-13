//! Assistants: AI agents that edit a document from outside the person's own hands (the control
//! channel behind `wordcraft-cli mcp --connect`). The rule, enforced here and not in the client:
//!
//! * every command that changes the document runs with track changes **forced on** and the
//!   revision (and comment) author set to the assistant's name, then the person's own tracking
//!   state and author are put back ([`Session::run_as_assistant`], or [`Session::enter_assistant`]
//!   / [`Session::leave_assistant`] around other work);
//! * an assistant cannot resolve its own work or loosen the rule: [`denied`] lists what it may not
//!   run (accept/reject, tracking off, author changes, undo/redo, macros, protection, and edits to
//!   other people's comments). Read-only commands are never denied.
//!
//! Limit: this guarantees that changes the engine records as revisions arrive as suggestions. A
//! few commands change things Word cannot track either (page setup, styles, document settings);
//! those still apply directly and show up only as undo steps.

use serde_json::Value;

use crate::{CmdError, CmdResult, Session};

/// Why a command is refused, when it is. `assistant` is the assistant's author name (comments it
/// wrote itself may be edited or deleted).
pub fn denied(s: &Session, id: &str, params: &Value, assistant: &str) -> Option<String> {
    let no = |why: &str| Some(format!("not allowed for assistants: {id} ({why})"));
    match id {
        _ if id.starts_with("review.accept") || id.starts_with("review.reject") => no("only the person reviewing accepts or rejects changes"),
        "review.trackChanges" => no("changes by assistants are always tracked"),
        "file.setAuthor" => no("the author of an assistant's changes is set in Settings › Assistant"),
        "file.options" if params.get("author").is_some() || params.get("initials").is_some() => {
            no("the author of an assistant's changes is set in Settings › Assistant")
        }
        // Settings, provider, key, switch, connection tests and model lists (Settings › Assistant).
        _ if id == "ui.assistant" || id.starts_with("ui.assistant.") || id.starts_with("assistant.") => {
            no("assistant settings, keys and connections belong to the person")
        }
        "edit.undo" | "edit.redo" | "edit.undoTo" | "edit.repeat" => {
            no("it could undo or repeat the person's own work; reject a suggestion in Review instead")
        }
        "tools.macros" | "tools.recordMacro" => no("a macro replays commands outside this check"),
        "review.restrict" => no("document protection is the person's to change"),
        "insert.bookmark" if params.get("name").and_then(Value::as_str).is_some_and(crate::authorship::is_mark_name) => {
            no("authorship marks are written only when the person accepts a suggestion")
        }
        "review.newComment" | "review.reply" if !s.assistant_comments => {
            no("the person has turned off comments by assistants in Settings › Assistant")
        }
        "review.editComment" if !s.assistant_comments => no("the person has turned off comments by assistants in Settings › Assistant"),
        "review.deleteComment" | "review.editComment" => {
            let own = params
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .and_then(|n| s.doc.comments.get(&n))
                .is_some_and(|c| c.author == assistant);
            if own { None } else { no("an assistant may only change comments it wrote") }
        }
        _ => None,
    }
}

/// Commands that swap the whole document: the person's tracking state belongs to the old one.
fn replaces_document(id: &str) -> bool {
    id.starts_with("file.new") || id.starts_with("file.open") || id == "file.close"
}

/// The person's author and tracking state, kept while an assistant works.
pub struct AssistantScope {
    author: String,
    track: bool,
}

impl Session {
    /// Start an assistant's command `id`: force tracking on and the author to `name`. `None` for a
    /// command that doesn't change the document (nothing is forced, nothing to restore). Pair
    /// with [`Session::leave_assistant`].
    pub fn enter_assistant(&mut self, name: &str, id: &str) -> Option<AssistantScope> {
        let mutates = self.registry.get(id).is_none_or(|c| c.mutates);
        if !mutates && !replaces_document(id) {
            return None;
        }
        self.add_assistant_author(name);
        let scope = AssistantScope { author: std::mem::replace(&mut self.author, name.to_string()), track: self.doc.settings.track_changes };
        self.doc.settings.track_changes = true;
        Some(scope)
    }

    /// Put the person's author and tracking state back (after a command that swapped the
    /// document, the new document keeps its own tracking setting).
    pub fn leave_assistant(&mut self, scope: Option<AssistantScope>, id: &str) {
        if let Some(scope) = scope {
            self.author = scope.author;
            if !replaces_document(id) {
                self.doc.settings.track_changes = scope.track;
            }
        }
    }

    /// Run `id` the way an outside assistant does: refused when [`denied`], otherwise with
    /// tracking forced on under `name` for commands that change the document.
    pub fn run_as_assistant(&mut self, name: &str, id: &str, params: &Value) -> CmdResult {
        if let Some(why) = denied(self, id, params, name) {
            return Err(CmdError::Failed(why));
        }
        let scope = self.enter_assistant(name, id);
        let r = self.run(id, params);
        self.leave_assistant(scope, id);
        r
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::Document;

    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new(Document::new());
        s.author = "Martijn".into();
        s.run("text.insert", &json!({"text": "Hello world"})).unwrap();
        s
    }

    fn authors(s: &mut Session) -> Vec<String> {
        let list = s.run("review.changes", &json!({})).unwrap();
        list.as_array().unwrap().iter().filter_map(|c| c["author"].as_str().map(str::to_string)).collect()
    }

    #[test]
    fn an_assistant_edit_is_a_tracked_change_and_the_persons_state_returns() {
        let mut s = session();
        assert!(!s.doc.settings.track_changes);
        s.run_as_assistant("Claude", "text.insert", &json!({"text": " again"})).unwrap();
        assert_eq!(authors(&mut s), vec!["Claude".to_string()]);
        // The person's own state is back: tracking off, their name, and their next edit is plain.
        assert!(!s.doc.settings.track_changes);
        assert_eq!(s.author, "Martijn");
        s.run("text.insert", &json!({"text": "!"})).unwrap();
        assert_eq!(authors(&mut s), vec!["Claude".to_string()]);
        // Tracking the person turned on stays on.
        s.run("review.trackChanges", &json!({"value": true})).unwrap();
        s.run_as_assistant("Claude", "text.insert", &json!({"text": "?"})).unwrap();
        assert!(s.doc.settings.track_changes);
    }

    #[test]
    fn accepting_and_rejecting_are_refused_and_change_nothing() {
        let mut s = session();
        s.run_as_assistant("Claude", "text.insert", &json!({"text": " again"})).unwrap();
        for id in ["review.acceptAll", "review.rejectAll", "review.accept", "review.reject"] {
            let e = s.run_as_assistant("Claude", id, &json!({})).unwrap_err().to_string();
            assert!(e.starts_with("not allowed for assistants: "), "{id}: {e}");
        }
        assert_eq!(authors(&mut s), vec!["Claude".to_string()]);
        assert!(s.doc.plain_text(wordcraft_doc::StoryRef::Body).contains("again"));
    }

    #[test]
    fn tracking_and_author_cannot_be_changed_by_an_assistant() {
        let mut s = session();
        for (id, p) in [
            ("review.trackChanges", json!({"value": false})),
            ("review.trackChanges", json!({})),
            ("file.setAuthor", json!({"name": "Someone"})),
            ("file.options", json!({"author": "Someone"})),
            ("file.options", json!({"initials": "S"})),
            ("ui.assistant", json!({"authorName": "Someone"})),
            ("ui.assistant", json!({"apiKey": "sk-x", "enabled": true})),
            ("ui.assistant.test", json!({})),
            ("ui.assistant.models", json!({"refresh": true})),
            ("assistant.test", json!({})),
            ("edit.undo", json!({})),
            ("tools.macros", json!({"run": "x"})),
            ("review.restrict", json!({})),
        ] {
            let e = s.run_as_assistant("Claude", id, &p).unwrap_err().to_string();
            assert!(e.starts_with("not allowed for assistants: "), "{id}: {e}");
        }
        assert_eq!(s.author, "Martijn");
        assert!(!s.doc.settings.track_changes);
        // Other settings through the same command stay possible.
        s.run_as_assistant("Claude", "file.options", &json!({"checkSpelling": true})).unwrap();
    }

    #[test]
    fn a_comment_is_authored_by_the_assistant_and_only_its_own_can_be_changed() {
        let mut s = session();
        s.run("review.newComment", &json!({"text": "mine"})).unwrap();
        s.run_as_assistant("Claude", "review.newComment", &json!({"text": "note"})).unwrap();
        let list = s.run("review.comments", &json!({})).unwrap();
        let list = list.as_array().unwrap();
        let by = |a: &str| list.iter().find(|c| c["author"] == a).map(|c| c["id"].clone()).unwrap();
        assert_eq!(list.len(), 2);
        assert!(s.run_as_assistant("Claude", "review.deleteComment", &json!({"id": by("Martijn")})).is_err());
        assert!(s.run_as_assistant("Claude", "review.deleteComment", &json!({"all": true})).is_err());
        assert!(s.run_as_assistant("Claude", "review.editComment", &json!({"id": by("Martijn"), "text": "x"})).is_err());
        s.run_as_assistant("Claude", "review.editComment", &json!({"id": by("Claude"), "text": "better"})).unwrap();
        s.run_as_assistant("Claude", "review.deleteComment", &json!({"id": by("Claude")})).unwrap();
        assert_eq!(s.run("review.comments", &json!({})).unwrap().as_array().unwrap().len(), 1);
    }

    #[test]
    fn the_author_is_the_name_given_each_time() {
        let mut s = session();
        s.run_as_assistant("Claude", "text.insert", &json!({"text": "a"})).unwrap();
        s.run_as_assistant("Claude (copyedit)", "text.insert", &json!({"text": "b"})).unwrap();
        let a = authors(&mut s);
        assert!(a.contains(&"Claude".to_string()) && a.contains(&"Claude (copyedit)".to_string()), "{a:?}");
        assert_eq!(s.author, "Martijn");
    }

    #[test]
    fn read_only_commands_run_and_a_failed_edit_still_restores() {
        let mut s = session();
        s.run_as_assistant("Claude", "review.changes", &json!({})).unwrap();
        s.run_as_assistant("Claude", "document.inspect", &json!({})).unwrap();
        assert!(!s.doc.settings.track_changes);
        assert!(s.run_as_assistant("Claude", "no.such.command", &json!({})).is_err());
        assert!(s.run_as_assistant("Claude", "text.insert", &json!({})).is_err());
        assert!(!s.doc.settings.track_changes);
        assert_eq!(s.author, "Martijn");
    }

    #[test]
    fn opening_another_document_keeps_that_documents_own_tracking_setting() {
        let mut s = session();
        s.run("review.trackChanges", &json!({"value": true})).unwrap();
        s.run_as_assistant("Claude", "file.new", &json!({"template": "blank"})).unwrap();
        assert_eq!(s.author, "Martijn");
        // The new document is not tracked: the old document's setting was not carried over.
        assert!(!s.doc.settings.track_changes);
    }

    #[test]
    fn with_comments_off_assistants_cannot_add_or_reply_but_the_person_can() {
        let mut s = session();
        s.run_as_assistant("Claude", "review.newComment", &json!({"text": "first"})).unwrap();
        let id = s.run("review.comments", &json!({})).unwrap()[0]["id"].clone();
        s.assistant_comments = false;
        for (cmd, p) in [
            ("review.newComment", json!({"text": "x"})),
            ("review.reply", json!({"id": id, "text": "x"})),
            ("review.editComment", json!({"id": id, "text": "x"})),
        ] {
            let e = s.run_as_assistant("Claude", cmd, &p).unwrap_err().to_string();
            assert!(e.starts_with("not allowed for assistants: ") && e.contains("comments"), "{cmd}: {e}");
        }
        // Its own earlier comment can still be removed, and the person's commands are not affected.
        s.run("review.newComment", &json!({"text": "mine"})).unwrap();
        s.run("review.reply", &json!({"id": id, "text": "thanks"})).unwrap();
        s.run_as_assistant("Claude", "review.deleteComment", &json!({"id": id})).unwrap();
        // Tracked edits are not comments.
        s.run_as_assistant("Claude", "text.insert", &json!({"text": "!"})).unwrap();
        s.assistant_comments = true;
        s.run_as_assistant("Claude", "review.newComment", &json!({"text": "again"})).unwrap();
    }
}
