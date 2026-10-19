//! Research (O8): findings with sources; a footnote with its source, or a marked quote. Never plain prose.
//!
//! Not built yet: O8 replaces this file and sets `AVAILABLE`.

use super::{Call, Output, Prompt, Ran, Reply, Verb, not_yet};
use crate::Session;
use crate::assistant_edits::{Applied, ScopeText};

pub(super) const AVAILABLE: bool = false;

pub(super) fn prompt(_c: &Call) -> Result<Option<Prompt>, String> {
    Err(not_yet(Verb::Research))
}

pub(super) fn read(_c: &Call, _reply: Option<Reply>) -> Result<Ran, String> {
    Err(not_yet(Verb::Research))
}

/// Land one of Footnote, Quote.
pub(super) fn land(_s: &mut Session, _name: &str, _scope: &ScopeText, _o: &Output) -> Result<Vec<Applied>, String> {
    Err(not_yet(Verb::Research))
}
