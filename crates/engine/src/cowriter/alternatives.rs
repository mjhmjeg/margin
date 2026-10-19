//! Alternatives (O5): up to three rewrites of the selected sentences; the one picked lands as one tracked replacement.
//!
//! Not built yet: O5 replaces this file and sets `AVAILABLE`.

use super::{Call, Output, Prompt, Ran, Reply, Verb, not_yet};
use crate::Session;
use crate::assistant_edits::{Applied, ScopeText};

pub(super) const AVAILABLE: bool = false;

pub(super) fn prompt(_c: &Call) -> Result<Option<Prompt>, String> {
    Err(not_yet(Verb::Alternatives))
}

pub(super) fn read(_c: &Call, _reply: Option<Reply>) -> Result<Ran, String> {
    Err(not_yet(Verb::Alternatives))
}

/// Land one of Replacement.
pub(super) fn land(_s: &mut Session, _name: &str, _scope: &ScopeText, _o: &Output) -> Result<Vec<Applied>, String> {
    Err(not_yet(Verb::Alternatives))
}
