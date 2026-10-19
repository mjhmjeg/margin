//! Read as a reader (O6): comments by the AI author, with optional one-click options.
//!
//! Not built yet: O6 replaces this file and sets `AVAILABLE`.

use super::{Call, Prompt, Ran, Reply, Verb, not_yet};

pub(super) const AVAILABLE: bool = false;

pub(super) fn prompt(_c: &Call) -> Result<Option<Prompt>, String> {
    Err(not_yet(Verb::ReadAsReader))
}

pub(super) fn read(_c: &Call, _reply: Option<Reply>) -> Result<Ran, String> {
    Err(not_yet(Verb::ReadAsReader))
}
