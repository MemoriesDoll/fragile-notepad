//! Owns the unsaved-document prompt from presentation through deferred dismissal.

use std::time::Instant;

use crate::core::{DirtyCloseDecision, DocumentId};

use super::animation::RevealAnimation;

#[derive(Debug, Clone, Copy)]
enum State {
    Hidden,
    Open(DocumentId),
    Closing {
        document: DocumentId,
        decision: DirtyCloseDecision,
    },
}

#[derive(Debug)]
pub(super) struct ClosePrompt {
    state: State,
    animation: RevealAnimation,
}

impl ClosePrompt {
    pub(super) const fn new() -> Self {
        Self {
            state: State::Hidden,
            animation: RevealAnimation::hidden(),
        }
    }

    pub(super) fn document(&self) -> Option<DocumentId> {
        match self.state {
            State::Hidden => None,
            State::Open(document) | State::Closing { document, .. } => Some(document),
        }
    }

    pub(super) fn is_closing(&self) -> bool {
        matches!(self.state, State::Closing { .. })
    }

    pub(super) fn show(&mut self, document: DocumentId) {
        if !self.is_closing() {
            self.state = State::Open(document);
            self.animation.set_visible(true);
        }
    }

    /// Return an immediately applicable decision, or defer it until the fade ends.
    pub(super) fn resolve(
        &mut self,
        document: DocumentId,
        decision: DirtyCloseDecision,
    ) -> Option<DirtyCloseDecision> {
        if self.is_closing() || self.document().is_some_and(|pending| pending != document) {
            return None;
        }
        if self.document() == Some(document) {
            self.animation.set_visible(false);
            if self.animation.rendered_visible() {
                self.state = State::Closing { document, decision };
                return None;
            }
        }
        self.dismiss(document);
        Some(decision)
    }

    pub(super) fn finish(&mut self, document: DocumentId) -> Option<DirtyCloseDecision> {
        match self.state {
            State::Closing {
                document: pending,
                decision,
            } if pending == document && !self.animation.rendered_visible() => {
                self.dismiss(document);
                Some(decision)
            }
            _ => None,
        }
    }

    pub(super) fn dismiss(&mut self, document: DocumentId) {
        if self.document() == Some(document) {
            self.state = State::Hidden;
            self.animation = RevealAnimation::hidden();
        }
    }

    pub(super) fn update_frame(&mut self, at: Instant) -> Option<DocumentId> {
        self.animation.update_frame(at);
        (self.is_closing() && !self.animation.rendered_visible())
            .then(|| self.document())
            .flatten()
    }

    pub(super) fn needs_frames(&self) -> bool {
        self.animation.needs_frames()
    }
    pub(super) fn progress(&self) -> f32 {
        self.animation.progress()
    }
    pub(super) fn interactive(&self) -> bool {
        matches!(self.state, State::Open(_))
    }

    #[cfg(test)]
    pub(super) fn decision(&self) -> Option<DirtyCloseDecision> {
        match self.state {
            State::Closing { decision, .. } => Some(decision),
            _ => None,
        }
    }
}
