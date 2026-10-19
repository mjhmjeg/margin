//! I'm stuck (O9): questions as comments; optionally one rough first line as a tracked insert.
//!
//! Not built yet: O9 replaces this file and sets `AVAILABLE`.

use super::{Call, Output, Prompt, Ran, Reply, Verb, not_yet};
use crate::Session;
use crate::assistant_edits::{Applied, ScopeText};

pub(super) const AVAILABLE: bool = false;

pub(super) fn prompt(_c: &Call) -> Result<Option<Prompt>, String> {
    Err(not_yet(Verb::Stuck))
}

pub(super) fn read(_c: &Call, _reply: Option<Reply>) -> Result<Ran, String> {
    Err(not_yet(Verb::Stuck))
}

/// Land one of FirstLine.
pub(super) fn land(_s: &mut Session, _name: &str, _scope: &ScopeText, _o: &Output) -> Result<Vec<Applied>, String> {
    Err(not_yet(Verb::Stuck))
}
