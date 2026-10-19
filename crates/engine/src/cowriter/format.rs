//! Format for me (O4): tracked style, structure and typography changes; the runner's identity check takes everything back if a word would change.
//!
//! Not built yet: O4 replaces this file and sets `AVAILABLE`.

use super::{Call, Output, Prompt, Ran, Reply, Verb, not_yet};
use crate::Session;
use crate::assistant_edits::{Applied, ScopeText};

pub(super) const AVAILABLE: bool = false;

/// The commands a format change may run (a trailing `.` takes the whole family). The runner
/// refuses any other, and checks the words stay the same.
pub(super) const COMMANDS: &[&str] = &["para.style"];

pub(super) fn prompt(_c: &Call) -> Result<Option<Prompt>, String> {
    Err(not_yet(Verb::FormatForMe))
}

pub(super) fn read(_c: &Call, _reply: Option<Reply>) -> Result<Ran, String> {
    Err(not_yet(Verb::FormatForMe))
}

/// Land one of FormatChange.
pub(super) fn land(_s: &mut Session, _name: &str, _scope: &ScopeText, _o: &Output) -> Result<Vec<Applied>, String> {
    Err(not_yet(Verb::FormatForMe))
}
