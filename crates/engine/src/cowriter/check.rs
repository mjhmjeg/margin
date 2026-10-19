//! Check facts and names (O6): comments with options; the chosen option becomes a tracked change.
//!
//! Not built yet: O6 replaces this file and sets `AVAILABLE`.

use super::{Call, Prompt, Ran, Reply, Verb, not_yet};

pub(super) const AVAILABLE: bool = false;

pub(super) fn prompt(_c: &Call) -> Result<Option<Prompt>, String> {
    Err(not_yet(Verb::CheckFacts))
}

pub(super) fn read(_c: &Call, _reply: Option<Reply>) -> Result<Ran, String> {
    Err(not_yet(Verb::CheckFacts))
}
