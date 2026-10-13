//! Authorship (Margin A5), read-only: the spans of accepted assistant text and the document's
//! split between people, assistants and pending suggestions. The view switch is
//! `view.authorship`; marks are only ever written by accepting a suggestion (`review.accept*`).

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;

use crate::{CmdResult, CommandSpec, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("authorship.spans", "Authorship Spans", "Agents", spans).pure(),
        CommandSpec::new("authorship.shares", "Authorship Shares", "Agents", shares).params(r#"{"paragraphs"?: bool}"#).pure(),
    ]
}

fn story_json(s: StoryRef) -> Value {
    match s {
        StoryRef::Body => json!("body"),
        StoryRef::Part(id) => json!(id),
    }
}

/// Every span: its mark, author, model, when it was accepted, how many characters it holds now
/// and how many of them are still the assistant's.
fn spans(s: &mut Session, _: &Value) -> CmdResult {
    let a = s.authorship();
    let list: Vec<Value> = a
        .spans
        .iter()
        .map(|st| {
            let entry = s.doc.authorship.get(&st.name);
            json!({
                "name": st.name,
                "author": st.author,
                "model": entry.map(|e| e.model.clone()).unwrap_or_default(),
                "acceptedAt": entry.map(|e| e.accepted_at.clone()).unwrap_or_default(),
                "story": story_json(st.start.story),
                "path": st.start.path.0,
                "start": st.start.off,
                "end": st.end.off,
                "chars": st.live_chars,
                "aiChars": st.ai_chars,
                "edited": st.edited,
            })
        })
        .collect();
    Ok(json!(list))
}

/// Characters by origin: `people`, `ai` (accepted, per author under `authors`), `pending`
/// (assistant suggestions not yet accepted, counted separately), `total`; with `paragraphs`,
/// the same per paragraph of the body.
fn shares(s: &mut Session, v: &Value) -> CmdResult {
    let a = s.authorship();
    let mut out = json!({
        "total": a.total_chars,
        "people": a.people_chars(),
        "ai": a.ai_chars,
        "pending": a.pending_chars,
        "authors": a.authors,
    });
    if crate::p::bool(v, "paragraphs") == Some(true) {
        let paras: Vec<Value> = a
            .paras
            .get(&StoryRef::Body)
            .map(|m| {
                m.iter()
                    .map(|(path, ps)| {
                        json!({
                            "path": path.0,
                            "total": ps.total_chars,
                            "ai": ps.ai_chars,
                            "pending": ps.pending_chars,
                            "author": ps.main_author(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out["paragraphs"] = json!(paras);
    }
    Ok(out)
}
