mod test_support {
    pub(super) use crate::app::windowing::ManagedWindow;
    pub(super) use crate::app::{App, CloseGoal, SYNTAX_PREWARM_VISIBLE_LINES};
    pub(super) use crate::core::{HardwareAccelerationMode, IndentationMode};
    pub(super) use crate::editor::{EditorAction, EditorBuffer, EditorPosition, EditorSelection};
    pub(super) use crate::message::{
        AboutTab, ClipboardMode, DirtyCloseDecision, Menu, Message, OpenedFile, PasteRequest,
        SaveRequest,
    };
    pub(super) use std::path::PathBuf;
    pub(super) use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

    pub(super) fn pending_save_all_ids(app: &App) -> Vec<crate::core::DocumentId> {
        app.pending_save_all.iter().copied().collect()
    }

    pub(super) fn set_active_document_text(app: &mut App, text: &str, selection: EditorSelection) {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = EditorBuffer::from_text(text);
        document.selection = selection;
        document.refresh_after_text_change();
        document.mark_clean();
    }
}

#[path = "app_tests/documents.rs"]
mod documents;
#[path = "app_tests/editor_actions.rs"]
mod editor_actions;
#[path = "app_tests/lifecycle.rs"]
mod lifecycle;
#[path = "app_tests/search.rs"]
mod search;
