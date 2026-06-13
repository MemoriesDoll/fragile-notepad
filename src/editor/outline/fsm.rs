use super::{OutlineBodyKind, OutlineNodeKind, OutlineRulePlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub(super) const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuralEventKind {
    Body {
        owner_kind: OutlineNodeKind,
        body_kind: OutlineBodyKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructuralEvent {
    pub kind: StructuralEventKind,
    pub keyword_range: ByteRange,
    pub name_range: ByteRange,
    pub signature_range: ByteRange,
    pub body_range: Option<ByteRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DeclarationEvent {
    pub rule: OutlineRulePlan,
    pub name: String,
    pub name_range: ByteRange,
    pub signature_range: ByteRange,
    pub body_range: Option<ByteRange>,
    pub terminated: bool,
}
