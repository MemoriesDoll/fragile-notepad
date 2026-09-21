use super::App;
use crate::core::session::{Session, SessionDocument};
use crate::core::{Document, DocumentId, DocumentLoadState, EditorSettings};
use crate::editor::{EditorBuffer, EditorPosition, EditorSelection};
use crate::message::{FileLoadRequest, Message};
use crate::startup::StartupOptions;
use iced::Task;
use std::{collections::HashMap, path::PathBuf, time::Duration};

#[derive(Debug)]
pub(super) struct SessionState {
    pub enabled: bool,
    pub ready: bool,
    pub loaded: bool,
    pub initialized: bool,
    pub exiting: bool,
    pub flush_scheduled: bool,
    pub dirty: bool,
    pub saving: bool,
    pub read_failed: bool,
    pub saved: Option<Session>,
    pub paths: Vec<PathBuf>,
    pub pending: HashMap<DocumentId, SessionDocument>,
    pub folds: HashMap<DocumentId, Vec<(usize, usize)>>,
}

impl SessionState {
    pub fn new(options: StartupOptions) -> Self {
        let ready = !options.restore_session && options.files.is_empty();
        Self {
            enabled: options.restore_session,
            ready,
            loaded: !options.restore_session,
            initialized: false,
            exiting: false,
            flush_scheduled: false,
            dirty: false,
            saving: false,
            read_failed: false,
            saved: None,
            paths: options.files,
            pending: HashMap::new(),
            folds: HashMap::new(),
        }
    }
}

impl App {
    pub(super) fn open_paths(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        if !self.settings_loaded || (self.session.enabled && !self.session.initialized) {
            self.session.paths.extend(paths);
            return Task::none();
        }
        let placeholder = self
            .workspace
            .active_document()
            .filter(|doc| {
                self.workspace.documents.len() == 1
                    && doc.path.is_none()
                    && !doc.is_dirty
                    && doc.buffer.len_bytes() == 0
            })
            .map(|doc| doc.id);
        let tasks = paths
            .into_iter()
            .map(|path| self.start_loading_file(path))
            .collect::<Vec<_>>();
        if !tasks.is_empty()
            && let Some(id) = placeholder
        {
            self.workspace.close(id);
            self.outline_states.remove(&id);
        }
        // Poll the active (last requested) file first while retaining tab order.
        Task::batch(tasks.into_iter().rev())
    }

    pub(super) fn session_loaded(
        &mut self,
        result: Result<Option<Session>, String>,
    ) -> Task<Message> {
        self.session.loaded = true;
        match result {
            Ok(session) => self.session.saved = session,
            Err(error) => {
                self.session.read_failed = true;
                self.file_status = Some(format!("Session could not be restored: {error}"));
            }
        }
        self.restore_startup()
    }

    pub(super) fn restore_startup(&mut self) -> Task<Message> {
        if self.session.initialized
            || !self.session.ready
            || !self.session.loaded
            || !self.settings_loaded
        {
            return Task::none();
        }
        self.session.initialized = true;
        let mut tasks = Vec::new();
        if let Some(saved) = self.session.saved.take() {
            let untouched = self.workspace.documents.len() == 1
                && self
                    .workspace
                    .active_document()
                    .is_some_and(|d| d.path.is_none() && !d.is_dirty && d.buffer.len_bytes() == 0);
            if !saved.documents.is_empty() {
                if untouched {
                    self.workspace.documents.clear();
                    self.outline_states.clear();
                }
                let mut ids = Vec::new();
                for entry in saved.documents {
                    let id = self.workspace.generate_document_id();
                    let generation = crate::core::DocumentLoadGeneration::next();
                    let mut document =
                        Document::loading(id, entry.path.clone().unwrap_or_default(), generation);
                    document.path = entry.path.clone();
                    document.load_state = DocumentLoadState::Deferred { generation };
                    document.defer_analysis = true;
                    document.is_pinned = entry.is_pinned;
                    document.is_dirty = entry.is_dirty;
                    document.set_decoration_settings(self.settings.decoration_settings());
                    document.set_word_wrap(self.settings.word_wrap);
                    self.workspace.documents.push(document);
                    self.session.pending.insert(id, entry);
                    ids.push(id);
                }
                let id = ids[saved.active_index.min(ids.len() - 1)];
                self.workspace.select(id);
                tasks.push(self.activate_document(id));
            }
        }
        let paths = std::mem::take(&mut self.session.paths);
        tasks.push(self.open_paths(paths));
        Task::batch(tasks)
    }

    pub(super) fn activate_document(&mut self, id: DocumentId) -> Task<Message> {
        let Some(document) = self.workspace.document_mut(id) else {
            return Task::none();
        };
        let DocumentLoadState::Deferred { generation } = document.load_state else {
            return Task::none();
        };
        document.load_state = DocumentLoadState::Loading {
            generation,
            bytes_read: 0,
            total_bytes: None,
        };
        if let Some(entry) = self.session.pending.get(&id)
            && entry.text.is_some()
        {
            let text = entry.text.as_ref().unwrap();
            document.buffer = EditorBuffer::from_text(text.clone());
            document.complete_streaming_load(generation, entry.encoding);
            self.apply_session_metadata(id);
            self.refresh_find_matches();
            return self.schedule_outline_parse(id);
        }
        let Some(path) = document.path.clone() else {
            document.complete_streaming_load(generation, crate::core::TextEncoding::Utf8);
            self.apply_session_metadata(id);
            return Task::none();
        };
        self.start_load_request(FileLoadRequest {
            document_id: id,
            generation,
            path,
            chunk_size: crate::services::DEFAULT_CHUNK_SIZE,
        })
    }

    pub(super) fn apply_session_metadata(&mut self, id: DocumentId) {
        let Some(entry) = self.session.pending.remove(&id) else {
            return;
        };
        let Some(document) = self.workspace.document_mut(id) else {
            return;
        };
        if entry.text.is_some() {
            document.encoding = entry.encoding;
            document.line_ending = entry
                .line_ending
                .as_deref()
                .and_then(crate::core::document::detect_line_ending);
            if entry.is_dirty {
                document.mark_dirty();
            }
        }
        document.restore_syntax(entry.syntax_token, entry.syntax_automatic);
        document.set_main_selection(EditorSelection::new(
            EditorPosition::new(entry.anchor_line, entry.anchor_column),
            EditorPosition::new(entry.cursor_line, entry.cursor_column),
        ));
        document.restore_session_scroll(
            entry
                .first_visible_position
                .map(|(line, column)| EditorPosition::new(line, column)),
            entry.first_visible_row,
            entry.horizontal_offset,
            !entry.collapsed_folds.is_empty() && document.can_run_full_document_analysis(),
        );
        self.session.folds.insert(id, entry.collapsed_folds);
    }

    pub(super) fn snapshot_session(&self) -> Session {
        let mut session = Session::default();
        session.active_index = self
            .workspace
            .documents
            .iter()
            .position(|d| d.id == self.workspace.active_document_id)
            .unwrap_or(0);
        session.documents = self
            .workspace
            .documents
            .iter()
            .map(|document| {
                if let Some(pending) = self.session.pending.get(&document.id)
                    && (matches!(document.load_state, DocumentLoadState::Deferred { .. })
                        || !document.is_dirty)
                {
                    let mut entry = pending.clone();
                    entry.is_pinned = document.is_pinned;
                    return entry;
                }
                let selection = document.main_selection();
                SessionDocument {
                    path: document.path.clone(),
                    text: ((document.has_complete_text_index()
                        || matches!(document.load_state, DocumentLoadState::Failed { .. }))
                        && (document.path.is_none() || document.is_dirty))
                        .then(|| document.text()),
                    encoding: document.encoding,
                    line_ending: document
                        .line_ending
                        .map(|ending| ending.as_str().to_owned()),
                    is_pinned: document.is_pinned,
                    is_dirty: document.is_dirty,
                    anchor_line: selection.anchor.line,
                    anchor_column: selection.anchor.column,
                    cursor_line: selection.cursor.line,
                    cursor_column: selection.cursor.column,
                    first_visible_row: document.scroll.first_visible_row,
                    first_visible_position: document
                        .session_top_position()
                        .map(|position| (position.line, position.column)),
                    horizontal_offset: document.scroll.horizontal_px,
                    syntax_token: Some(document.syntax_token.clone()),
                    syntax_automatic: Some(document.syntax_is_automatic()),
                    collapsed_folds: self
                        .session
                        .folds
                        .get(&document.id)
                        .cloned()
                        .unwrap_or_else(|| {
                            document
                                .folds
                                .collapsed_ranges()
                                .map(|r| (r.start_line, r.end_line))
                                .collect()
                        }),
                }
            })
            .collect();
        session
    }

    pub(super) fn session_should_track(&self, message: &Message) -> bool {
        #[cfg(debug_assertions)]
        if matches!(message, Message::ToggleTitleBarStyle) {
            return false;
        }
        if let Message::RuntimeEvent(event, _, _) = message {
            return self.session.enabled
                && matches!(
                    event,
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. })
                );
        }
        self.session.enabled
            && !matches!(
                message,
                Message::None
                    | Message::WindowChrome(..)
                    | Message::WindowMaximized(..)
                    | Message::SyntaxParsed(..)
                    | Message::SessionFlush
                    | Message::SessionPersisted(_)
                    | Message::ShutdownPersisted(_)
                    | Message::RefreshLoadingFind
                    | Message::SettingsFlush
                    | Message::SettingsPersisted(_)
                    | Message::RuntimeEvent(..)
                    | Message::ChromeAnimationFrame(_)
                    | Message::FileLoadProgress(_)
                    | Message::FileLoadChunk(_)
                    | Message::EditorAction(_, crate::editor::EditorAction::ViewportChanged { .. })
            )
    }

    pub(super) fn request_session_save(&mut self) -> Task<Message> {
        self.session.dirty = true;
        if !self.session.enabled
            || !self.session.initialized
            || self.session.read_failed
            || self.session.flush_scheduled
            || self.session.saving
            || self.session.exiting
        {
            return Task::none();
        }
        self.session.flush_scheduled = true;
        Task::perform(
            async {
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
            |_| Message::SessionFlush,
        )
    }

    pub(super) fn flush_session(&mut self) -> Task<Message> {
        self.session.flush_scheduled = false;
        if !self.session.dirty
            || self.session.saving
            || !self.session.initialized
            || self.session.read_failed
        {
            return Task::none();
        }
        self.session.dirty = false;
        self.session.saving = true;
        Task::perform(
            crate::services::session_store::save_session(self.snapshot_session()),
            Message::SessionPersisted,
        )
    }

    pub(super) fn session_persisted(&mut self, result: Result<(), String>) -> Task<Message> {
        self.session.saving = false;
        if let Err(error) = result {
            self.session.dirty = true;
            self.file_status = Some(format!("Session save failed: {error}"));
            return Task::none();
        }
        if self.session.dirty {
            self.request_session_save()
        } else {
            Task::none()
        }
    }

    pub(super) fn persist_before_exit(&mut self) -> Task<Message> {
        if !self.settings_loaded || !self.session.initialized {
            self.file_status = Some("Finish starting before closing.".into());
            return Task::none();
        }
        if self.session.read_failed {
            if self.workspace.documents.iter().any(|doc| doc.is_dirty) {
                self.file_status = Some("Save or close unsaved tabs before exiting; the unreadable previous session will be preserved.".into());
                return Task::none();
            }
            return self.exit_after_settings();
        }
        self.session.exiting = true;
        let settings = (!self.settings_read_failed)
            .then(|| crate::services::save_settings(self.settings.clone()));
        let session = crate::services::session_store::save_session(self.snapshot_session());
        let preserve_settings = self.settings_read_failed;
        Task::perform(
            async move {
                if let Some(settings) = settings {
                    settings
                        .await
                        .map_err(|e| format!("settings: {}", e.summary()))?;
                }
                session.await?;
                if preserve_settings {
                    Ok(())
                } else {
                    crate::services::flush_settings()
                        .await
                        .map_err(|e| e.summary().to_owned())
                }
            },
            Message::ShutdownPersisted,
        )
    }

    pub(super) fn shutdown_persisted(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(()) => iced::exit(),
            Err(error) => {
                self.session.exiting = false;
                self.file_status = Some(format!("Could not save session: {error}"));
                Task::none()
            }
        }
    }

    pub(super) fn exit_after_settings(&mut self) -> Task<Message> {
        if !self.settings_loaded || self.settings_read_failed {
            return iced::exit();
        }
        self.session.exiting = true;
        let save = crate::services::save_settings(self.settings.clone());
        Task::perform(
            async move { save.await.map_err(|error| error.summary().to_owned()) },
            Message::ShutdownPersisted,
        )
    }

    pub(super) fn schedule_active_analysis(&mut self) -> Task<Message> {
        if self.session.exiting {
            return Task::none();
        }
        if self.analysis_in_flight.is_some() {
            return Task::none();
        }
        let Some((buffer, request)) = self
            .workspace
            .active_document()
            .and_then(Document::analysis_request)
        else {
            return Task::none();
        };
        self.analysis_in_flight = Some((
            request.document_id,
            request.revision,
            request.syntax_token.clone(),
            request.indent_width,
        ));
        Task::perform(
            async move {
                let fallback = request.clone();
                tokio::task::spawn_blocking(move || {
                    crate::core::document::analyze_document(buffer, request)
                })
                .await
                .unwrap_or(fallback)
            },
            Message::DocumentAnalyzed,
        )
    }
}

pub(super) fn merge_initial_settings(
    current: &EditorSettings,
    mut loaded: EditorSettings,
    edits: u32,
) -> EditorSettings {
    let defaults = EditorSettings::default();
    macro_rules! keep_edits { ($($field:ident),*) => { $(if current.$field != defaults.$field { loaded.$field = current.$field.clone(); })* }; }
    keep_edits!(
        word_wrap,
        zoom,
        scroll_speed,
        indentation,
        appearance,
        hardware_acceleration,
        syntax_theme,
        shortcuts
    );
    if edits & 1 != 0 {
        loaded.zoom = current.zoom;
    }
    if edits & 2 != 0 {
        loaded.word_wrap = current.word_wrap;
    }
    macro_rules! decoration_edit {
        ($field:ident, $mask:expr) => {
            if edits & $mask != 0 || current.decorations.$field != defaults.decorations.$field {
                loaded.decorations.$field = current.decorations.$field;
            }
        };
    }
    decoration_edit!(show_line_numbers, 4);
    decoration_edit!(show_spaces, 8);
    decoration_edit!(show_tabs, 16);
    decoration_edit!(show_end_of_line_markers, 32);
    decoration_edit!(show_indentation_guides, 64);
    decoration_edit!(show_folding_controls, 128);
    if edits == u32::MAX {
        let history = loaded.open_history;
        loaded = current.clone();
        loaded.open_history = history;
    }
    for path in current.open_history.iter().rev() {
        loaded.record_open_history_path(path.clone());
    }
    loaded
}

pub(super) fn settings_edit_mask(message: &Message) -> u32 {
    match message {
        Message::ZoomIn
        | Message::ZoomOut
        | Message::ZoomReset
        | Message::Shortcut(
            crate::core::ShortcutCommand::ZoomIn
            | crate::core::ShortcutCommand::ZoomOut
            | crate::core::ShortcutCommand::ZoomReset,
        ) => 1,
        Message::ToggleWordWrap => 2,
        Message::ToggleLineNumbers => 4,
        Message::ToggleVisibleSpaces => 8,
        Message::ToggleVisibleTabs => 16,
        Message::ToggleSpaceAndTab => 8 | 16,
        Message::ToggleEolMarkers => 32,
        Message::ToggleAllCharacters => 8 | 16 | 32,
        Message::ToggleIndentationGuides => 64,
        Message::ToggleFoldingControls => 128,
        Message::ApplySettings | Message::SaveSettings => u32::MAX,
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
