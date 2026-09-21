//! User decisions about unsaved documents.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyCloseDecision {
    Save,
    Discard,
    Cancel,
}
