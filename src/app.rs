use iced::{Element, Subscription, Task, Theme, event, keyboard, stream, window};

use crate::core::{DocumentId, EditorSettings, FindState, Workspace};
use crate::editor::{
    EditorAction, EditorSelection, OutlineParseResult, OutlineSnapshotMetadata, OutlineState,
    outline_registry_hash, outline_request_for_document, parse_outline_request,
};
use crate::ipc::{PrimaryInstance, Signal};
use crate::message::{AboutTab, Menu, Message, SaveRequest};
use crate::search_dialog::SearchDialogState;
use crate::settings_dialog::SettingsDialogState;
use crate::ui;

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windowing::{AdvancedSearchWindow, ManagedWindow, SettingsWindow};

mod editor;
mod editor_ops;
mod files;
mod menu;
mod rendering;
mod search;
mod settings;
mod shortcuts;
mod windowing;

const SYNTAX_PREWARM_VISIBLE_LINES: usize = 96;
const ABOUT_OVERLAY_ANIMATION_DURATION: Duration = Duration::from_millis(180);

static SINGLE_INSTANCE: OnceLock<PrimaryInstance> = OnceLock::new();

#[derive(Debug)]
pub struct App {
    workspace: Workspace,
    find: FindState,
    settings: EditorSettings,
    outline_states: HashMap<DocumentId, OutlineState>,
    outline_registry_hash: u64,
    is_loading: bool,
    pending_save: Option<SaveRequest>,
    pending_save_all: VecDeque<crate::core::DocumentId>,
    pending_close_after_save: Option<crate::core::DocumentId>,
    pending_close_documents: VecDeque<crate::core::DocumentId>,
    pending_dirty_close: Option<crate::core::DocumentId>,
    close_goal: CloseGoal,
    file_status: Option<String>,
    is_find_visible: bool,
    is_inline_replace_visible: bool,
    is_function_list_visible: bool,
    main_window_id: Option<window::Id>,
    settings_window: Option<SettingsWindow>,
    advanced_search_window: Option<AdvancedSearchWindow>,
    active_menu: Option<Menu>,
    active_menu_path: Vec<String>,
    is_about_visible: bool,
    about_tab: AboutTab,
    is_window_list_visible: bool,
    settings_dialog: SettingsDialogState,
    search_dialog: SearchDialogState,
    dragged_tab: Option<crate::core::DocumentId>,
    hovered_drop_tab: Option<crate::core::DocumentId>,
    keyboard_modifiers: keyboard::Modifiers,
    focused_window_id: Option<window::Id>,
    rendering: rendering::RenderingState,
    about_animation: AboutOverlayAnimation,
    main_window_opened: bool,
    pending_startup_gpu_boost: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseGoal {
    KeepOpen,
    ExitApp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AboutOverlayAnimation {
    Idle,
    WaitingForHardware {
        boost_requested: bool,
    },
    Running {
        started_at: Option<Instant>,
        progress: f32,
    },
    Disabled,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let (main_window_id, open) = window::open(window::Settings {
            exit_on_close_request: false,
            ..window::Settings::default()
        });

        let outline_registry_hash = outline_registry_hash();
        let mut app = Self {
            workspace: Workspace::new(),
            find: FindState::new(),
            settings: EditorSettings::default(),
            outline_states: HashMap::new(),
            outline_registry_hash,
            is_loading: false,
            pending_save: None,
            pending_save_all: VecDeque::new(),
            pending_close_after_save: None,
            pending_close_documents: VecDeque::new(),
            pending_dirty_close: None,
            close_goal: CloseGoal::KeepOpen,
            file_status: None,
            is_find_visible: false,
            is_inline_replace_visible: false,
            is_function_list_visible: false,
            main_window_id: Some(main_window_id),
            settings_window: None,
            advanced_search_window: None,
            active_menu: None,
            active_menu_path: Vec::new(),
            is_about_visible: false,
            about_tab: AboutTab::About,
            is_window_list_visible: false,
            settings_dialog: SettingsDialogState::new(&EditorSettings::default()),
            search_dialog: SearchDialogState::new(),
            dragged_tab: None,
            hovered_drop_tab: None,
            keyboard_modifiers: keyboard::Modifiers::default(),
            focused_window_id: Some(main_window_id),
            rendering: rendering::RenderingState::Software,
            about_animation: AboutOverlayAnimation::Idle,
            main_window_opened: false,
            pending_startup_gpu_boost: false,
        };

        app.refresh_find_matches();
        let outline_task = app.schedule_outline_parse(app.workspace.active_document_id);

        (
            app,
            Task::batch([
                open.map(Message::WindowOpened),
                iced::widget::operation::focus(crate::ui::editor::EDITOR_ID),
                Task::perform(crate::services::load_settings(), Message::SettingsLoaded),
                outline_task,
            ]),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        if let Some(perf_span) = crate::perf_trace::span("app_update", format_args!("{message:?}"))
        {
            let task = self.update_inner(message);
            perf_span.end_with("");

            return task;
        }

        self.update_inner(message)
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::None => Task::none(),
            Message::SingleInstanceShowRequested(request) => self.show_main_window(request),
            Message::Shortcut(shortcut) => self.update_shortcut(shortcut),
            Message::RuntimeEvent(event, status, window_id) => {
                self.update_runtime_event(event, status, window_id)
            }
            Message::EditorAction(document_id, action) => self.update_editor(document_id, action),
            Message::OutlineParseCompleted(result) => self.complete_outline_parse(result),
            Message::ClipboardRead(request, result) => self.update_clipboard_read(request, result),
            Message::ClipboardWritten(_result) => Task::none(),
            Message::BackendBoostRequested => self.request_gpu_boost(),
            Message::BackendBoostConfigured(result) => self.complete_backend_boost(result),
            Message::AboutAnimationFrame(at) => self.update_about_animation_frame(at),
            Message::LanguageSelected(syntax_token) => self.update_language(syntax_token),
            Message::ToggleFunctionList => self.toggle_function_list(),
            Message::FunctionListEntrySelected(position) => {
                self.select_function_list_entry(position)
            }
            message @ (Message::MenuToggled(_)
            | Message::MenuHovered(_)
            | Message::MenuPathHovered(_)
            | Message::MenuClosed) => self.update_menu(message),
            Message::AboutOpened => self.open_about_dialog(),
            Message::AboutTabSelected(tab) => {
                self.about_tab = tab;
                Task::none()
            }
            Message::AboutClosed => {
                self.is_about_visible = false;
                self.about_animation = AboutOverlayAnimation::Idle;
                Task::none()
            }
            Message::WindowListOpened => {
                self.active_menu = None;
                self.active_menu_path.clear();
                self.is_window_list_visible = true;
                Task::none()
            }
            Message::WindowListClosed => {
                self.is_window_list_visible = false;
                Task::none()
            }
            Message::WindowFocusRequested(target) => self.focus_window(target),
            Message::WindowFocusNext => self.focus_adjacent_window(1),
            Message::WindowFocusPrevious => self.focus_adjacent_window(-1),
            message @ (Message::DraftThemeSelected(_)
            | Message::DraftWordWrapToggled(_)
            | Message::DraftAppearanceSelected(_)
            | Message::DraftHardwareAccelerationSelected(_)
            | Message::DraftIndentationSelected(_)
            | Message::DraftLineNumbersToggled(_)
            | Message::DraftVisibleSpacesToggled(_)
            | Message::DraftVisibleTabsToggled(_)
            | Message::DraftEolMarkersToggled(_)
            | Message::DraftIndentationGuidesToggled(_)
            | Message::DraftFoldingControlsToggled(_)
            | Message::SettingsCategorySelected(_)
            | Message::ShortcutGroupSelected(_)
            | Message::SettingsZoomIn
            | Message::SettingsZoomOut
            | Message::SettingsZoomReset
            | Message::SettingsScrollSpeedIncrease
            | Message::SettingsScrollSpeedDecrease
            | Message::SettingsScrollSpeedReset
            | Message::ApplySettings
            | Message::SaveSettings
            | Message::SettingsLoaded(_)
            | Message::SettingsPersisted(_)
            | Message::CancelSettings
            | Message::ToggleSettingsPanel
            | Message::ShortcutCaptureStarted(_)
            | Message::ShortcutCaptured(_, _)
            | Message::ShortcutCleared(_)
            | Message::ShortcutsResetToDefaults
            | Message::ShortcutConflictDismissed
            | Message::ShortcutCaptureConflict(_)
            | Message::ZoomIn
            | Message::ZoomOut
            | Message::ZoomReset
            | Message::ToggleWordWrap
            | Message::ToggleLineNumbers
            | Message::ToggleSpaceAndTab
            | Message::ToggleVisibleSpaces
            | Message::ToggleVisibleTabs
            | Message::ToggleEolMarkers
            | Message::ToggleAllCharacters
            | Message::ToggleIndentationGuides
            | Message::ToggleFoldingControls) => self.update_settings(message),
            Message::FoldCurrent => self.update_active_fold_command(EditorAction::FoldCurrent),
            Message::UnfoldCurrent => self.update_active_fold_command(EditorAction::UnfoldCurrent),
            Message::ToggleCurrentFold => {
                self.update_active_fold_command(EditorAction::ToggleCurrentFold)
            }
            Message::FoldAll => self.update_active_fold_command(EditorAction::FoldAll),
            Message::UnfoldAll => self.update_active_fold_command(EditorAction::UnfoldAll),
            Message::GoToMatchingDelimiter => {
                self.update_active_editor_command(EditorAction::GoToMatchingDelimiter)
            }
            Message::SelectMatchingDelimiter => {
                self.update_active_editor_command(EditorAction::SelectMatchingDelimiter)
            }
            Message::NextFunction => self.update_active_editor_command(EditorAction::NextFunction),
            Message::PreviousFunction => {
                self.update_active_editor_command(EditorAction::PreviousFunction)
            }
            Message::SelectCurrentFunction => {
                self.update_active_editor_command(EditorAction::SelectCurrentFunction)
            }
            Message::SelectCurrentFunctionBody => {
                self.update_active_editor_command(EditorAction::SelectCurrentFunctionBody)
            }
            Message::Cut => self.update_active_editor_command(EditorAction::Cut),
            Message::Copy => self.update_active_editor_command(EditorAction::Copy),
            Message::Paste => self.update_active_editor_command(EditorAction::Paste),
            Message::Delete => self.update_active_editor_command(EditorAction::Delete),
            message @ (Message::Undo | Message::Redo) => self.update_editor_command(message),
            message @ (Message::TabSelected(_)
            | Message::TabClosed(_)
            | Message::TabPinToggled(_)
            | Message::TabDragStarted(_)
            | Message::TabDragHovered(_)
            | Message::TabDragLeft(_)
            | Message::TabDragReleased(_)
            | Message::NewFile
            | Message::OpenFile
            | Message::FileDropped(_, _)
            | Message::FilePicked(_)
            | Message::FileOpened(_)
            | Message::FileLoadProgress(_)
            | Message::FileLoadChunk(_)
            | Message::FileLoadFinished(_)
            | Message::SaveFile
            | Message::SaveAllFiles
            | Message::SaveFileAs
            | Message::FileSaved(_, _)
            | Message::EncodingSelected(_)
            | Message::CloseFile
            | Message::CloseAllFiles
            | Message::CloseAllButActiveFile
            | Message::CloseAllButPinnedFiles
            | Message::CloseAllToLeft
            | Message::CloseAllToRight
            | Message::CloseAllUnchanged
            | Message::DirtyCloseResolved(_, _)) => self.update_file(message),
            message @ (Message::FindQueryChanged(_)
            | Message::FindReplacementChanged(_)
            | Message::FindCaseSensitiveToggled(_)
            | Message::FindWholeWordToggled(_)
            | Message::ToggleInlineReplace
            | Message::ShowInlineReplace
            | Message::ToggleFind
            | Message::HideFind
            | Message::FindNext
            | Message::FindPrevious
            | Message::SelectAndFindNext
            | Message::SelectAndFindPrevious
            | Message::VolatileFindNext
            | Message::VolatileFindPrevious
            | Message::ReplaceCurrent
            | Message::ReplaceAll
            | Message::ToggleAdvancedSearch(_)
            | Message::AdvancedSearchTabSelected(_)
            | Message::AdvancedSearchQueryChanged(_)
            | Message::AdvancedSearchReplacementChanged(_)
            | Message::AdvancedSearchCaseSensitiveToggled(_)
            | Message::AdvancedSearchWholeWordToggled(_)
            | Message::AdvancedSearchWrapAroundToggled(_)
            | Message::AdvancedSearchModeSelected(_)
            | Message::AdvancedSearchIncludeChanged(_)
            | Message::AdvancedSearchRun
            | Message::AdvancedCountRun
            | Message::AdvancedFindNextRun
            | Message::AdvancedFindAllCurrentRun
            | Message::AdvancedFindAllOpenRun
            | Message::AdvancedReplaceRun
            | Message::AdvancedReplaceAllRun
            | Message::AdvancedReplaceAllCurrentRun
            | Message::AdvancedReplaceAllOpenRun
            | Message::AdvancedSearchResultSelected(_, _)
            | Message::AdvancedSearchClosed) => self.update_search(message),
            Message::WindowOpened(id) => {
                if self.main_window_id == Some(id) {
                    self.main_window_opened = true;
                }

                let window_task = self.update_window(Message::WindowOpened(id));

                if self.main_window_id == Some(id) && self.pending_startup_gpu_boost {
                    self.pending_startup_gpu_boost = false;
                    Task::batch([window_task, self.request_gpu_boost()])
                } else {
                    window_task
                }
            }
            message @ (Message::WindowCloseRequested(_) | Message::WindowClosed(_)) => {
                self.update_window(message)
            }
        }
    }

    pub fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        let perf_span = crate::perf_trace::span("app_view", format_args!("window={window_id:?}"));

        crate::startup::report_first_view_ready();

        let element = if let Some(settings_window) = &self.settings_window
            && settings_window.is(window_id)
        {
            settings_window.view(&self.settings_dialog)
        } else if let Some(search_window) = &self.advanced_search_window
            && search_window.is(window_id)
        {
            search_window.view(&self.search_dialog)
        } else {
            ui::view(
                &self.workspace,
                &self.find,
                &self.settings,
                self.is_find_visible,
                self.is_inline_replace_visible,
                self.is_function_list_visible,
                self.active_menu,
                &self.active_menu_path,
                self.window_menu_state(),
                self.dragged_tab,
                self.hovered_drop_tab,
                self.pending_dirty_close
                    .and_then(|id| self.workspace.document(id)),
                self.is_about_visible.then_some(self.about_tab),
                ui::about_dialog::RenderingDebugInfo {
                    current_renderer: self.rendering.label(),
                    rendering_policy: rendering::current_policy_label(&self.settings),
                },
                self.about_animation_info(),
                self.is_window_list_visible
                    .then(|| self.window_list_entries()),
                self.file_status.as_deref(),
                self.active_outline_state(),
            )
        };
        if let Some(span) = perf_span {
            span.end_with("");
        }

        element
    }

    pub fn title(&self, window_id: window::Id) -> String {
        if let Some(settings_window) = &self.settings_window
            && settings_window.is(window_id)
        {
            return settings_window.title();
        }

        if let Some(search_window) = &self.advanced_search_window
            && search_window.is(window_id)
        {
            return search_window.title();
        }

        self.workspace
            .active_document()
            .map(|document| format!("{} - Fragile Notepad", document.title()))
            .unwrap_or_else(|| String::from("Fragile Notepad"))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let about_animation = if self.about_animation.needs_frames() {
            window::frames().map(Message::AboutAnimationFrame)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            single_instance_subscription(),
            event::listen_with(shortcuts::event_to_message),
            window::close_requests().map(Message::WindowCloseRequested),
            window::close_events().map(Message::WindowClosed),
            about_animation,
        ])
    }

    pub fn theme(&self, _window_id: window::Id) -> Theme {
        ui::styles::modern_theme(self.settings.appearance)
    }

    fn refresh_find_matches(&mut self) {
        if self.find.query.is_empty() {
            self.find.refresh_matches("");
            return;
        }

        let Some(document) = self.workspace.active_document() else {
            self.find.refresh_matches("");
            return;
        };

        self.find
            .refresh_matches_in_chunks(document.buffer.chunks());
    }

    fn prewarm_active_syntax_cache(&self) {
        let Some(document) = self.workspace.active_document() else {
            return;
        };

        if !document.uses_syntax_highlighting() {
            return;
        }

        if !document.has_complete_text_index() {
            return;
        }

        let first_row = document.scroll.first_visible_row;
        let first_line = document
            .viewport
            .visible_row_to_document_line(first_row)
            .unwrap_or(0);
        let last_row = first_row.saturating_add(SYNTAX_PREWARM_VISIBLE_LINES);
        let last_line = document
            .viewport
            .visible_row_to_document_line(last_row)
            .unwrap_or_else(|| document.buffer.line_count().saturating_sub(1));

        document.ensure_visible_syntax_cache(self.settings.syntax_theme, first_line, last_line);
    }

    fn active_outline_state(&self) -> Option<&OutlineState> {
        let document = self.workspace.active_document()?;
        let metadata = OutlineSnapshotMetadata::from_document(document, self.outline_registry_hash);

        self.outline_states
            .get(&document.id)
            .filter(|state| state.matches_metadata(&metadata))
    }

    fn schedule_outline_parse(&mut self, document_id: DocumentId) -> Task<Message> {
        let Some(document) = self.workspace.document(document_id) else {
            self.outline_states.remove(&document_id);
            return Task::none();
        };

        if !document.can_run_full_document_analysis() {
            let metadata =
                OutlineSnapshotMetadata::from_document(document, self.outline_registry_hash);
            self.outline_states
                .entry(document_id)
                .and_modify(|state| {
                    if !state.matches_metadata(&metadata) {
                        *state = OutlineState::pending_metadata(metadata.clone());
                    }
                })
                .or_insert_with(|| OutlineState::pending_metadata(metadata));
            return Task::none();
        }

        let request = outline_request_for_document(document, self.outline_registry_hash);
        let metadata = OutlineSnapshotMetadata::from_request(&request);

        if self
            .outline_states
            .get(&document_id)
            .is_some_and(|state| state.matches_metadata(&metadata))
        {
            return Task::none();
        }

        self.outline_states
            .insert(document_id, OutlineState::pending(&request));

        Task::perform(
            parse_outline_request(request),
            Message::OutlineParseCompleted,
        )
    }

    fn complete_outline_parse(&mut self, result: OutlineParseResult) -> Task<Message> {
        let metadata = OutlineSnapshotMetadata::from_result(&result);
        let Some(document) = self.workspace.document(metadata.document_id) else {
            self.outline_states.remove(&metadata.document_id);
            return Task::none();
        };

        if !document.can_run_full_document_analysis()
            || !metadata.matches_document(document, self.outline_registry_hash)
            || !self
                .outline_states
                .get(&metadata.document_id)
                .is_some_and(|state| state.matches_metadata(&metadata))
        {
            return Task::none();
        }

        self.outline_states
            .insert(metadata.document_id, OutlineState::ready(result));

        Task::none()
    }

    fn update_active_fold_command(&mut self, action: EditorAction) -> Task<Message> {
        self.update_active_editor_command(action)
    }

    fn update_active_editor_command(&mut self, action: EditorAction) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();

        self.update_editor(self.workspace.active_document_id, action)
    }

    fn toggle_function_list(&mut self) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();
        self.is_function_list_visible = !self.is_function_list_visible;

        Task::none()
    }

    fn open_about_dialog(&mut self) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();
        self.is_about_visible = true;
        self.about_tab = AboutTab::About;

        match self.rendering {
            rendering::RenderingState::Hardware => {
                self.start_about_animation();
                Task::none()
            }
            rendering::RenderingState::PreparingHardware => {
                self.about_animation = AboutOverlayAnimation::WaitingForHardware {
                    boost_requested: false,
                };
                Task::none()
            }
            rendering::RenderingState::Software
                if rendering::gpu_boost_policy_allows_hardware(&self.settings) =>
            {
                let should_request_boost = !matches!(
                    self.about_animation,
                    AboutOverlayAnimation::WaitingForHardware {
                        boost_requested: true
                    }
                );

                self.about_animation = AboutOverlayAnimation::WaitingForHardware {
                    boost_requested: true,
                };

                if should_request_boost {
                    Task::done(Message::BackendBoostRequested)
                } else {
                    Task::none()
                }
            }
            rendering::RenderingState::Software | rendering::RenderingState::Failed(_) => {
                self.about_animation = AboutOverlayAnimation::Disabled;
                Task::none()
            }
        }
    }

    fn complete_backend_boost(
        &mut self,
        outcome: iced::backend::StrictHandoffOutcome,
    ) -> Task<Message> {
        let task = self.complete_gpu_boost(outcome);

        if matches!(
            self.about_animation,
            AboutOverlayAnimation::WaitingForHardware { .. }
        ) {
            if self.rendering == rendering::RenderingState::Hardware {
                self.start_about_animation();
            } else {
                self.about_animation = AboutOverlayAnimation::Disabled;
            }
        }

        task
    }

    fn start_about_animation(&mut self) {
        self.about_animation = AboutOverlayAnimation::Running {
            started_at: None,
            progress: 0.0,
        };
    }

    fn update_about_animation_frame(&mut self, at: Instant) -> Task<Message> {
        let AboutOverlayAnimation::Running {
            started_at,
            progress,
        } = &mut self.about_animation
        else {
            return Task::none();
        };

        let started_at = match *started_at {
            Some(started_at) => started_at,
            None => {
                *started_at = Some(at);
                *progress = 0.0;
                return Task::none();
            }
        };

        let elapsed = at.saturating_duration_since(started_at);
        *progress =
            (elapsed.as_secs_f32() / ABOUT_OVERLAY_ANIMATION_DURATION.as_secs_f32()).min(1.0);

        Task::none()
    }

    fn about_animation_info(&self) -> ui::about_dialog::AboutAnimationInfo {
        self.about_animation.into()
    }

    fn select_function_list_entry(
        &mut self,
        position: crate::editor::EditorPosition,
    ) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();

        let Some(document) = self.workspace.active_document_mut() else {
            return Task::none();
        };

        let position = document.buffer.clamp_position(position);
        document.selection = EditorSelection::new(position, position);
        document.preferred_vertical_column = None;
        document.reveal_line(position.line);

        Task::none()
    }
}

impl AboutOverlayAnimation {
    fn needs_frames(self) -> bool {
        matches!(self, Self::Running { progress, .. } if progress < 1.0)
    }
}

impl From<AboutOverlayAnimation> for ui::about_dialog::AboutAnimationInfo {
    fn from(animation: AboutOverlayAnimation) -> Self {
        match animation {
            AboutOverlayAnimation::Idle => Self {
                progress: 0.0,
                visual_progress: 1.0,
                status: "idle",
                is_animating: false,
            },
            AboutOverlayAnimation::WaitingForHardware { .. } => Self {
                progress: 0.0,
                visual_progress: 0.0,
                status: "waiting for hardware",
                is_animating: false,
            },
            AboutOverlayAnimation::Running {
                started_at: None,
                progress,
            } => Self {
                progress,
                visual_progress: progress,
                status: "armed after hardware",
                is_animating: true,
            },
            AboutOverlayAnimation::Running {
                started_at: Some(_),
                progress,
            } if progress >= 1.0 => Self {
                progress,
                visual_progress: 1.0,
                status: "complete",
                is_animating: false,
            },
            AboutOverlayAnimation::Running { progress, .. } => Self {
                progress,
                visual_progress: progress,
                status: "running",
                is_animating: true,
            },
            AboutOverlayAnimation::Disabled => Self {
                progress: 0.0,
                visual_progress: 1.0,
                status: "disabled",
                is_animating: false,
            },
        }
    }
}

pub fn register_single_instance(instance: PrimaryInstance) {
    let _ = SINGLE_INSTANCE.set(instance);
}

fn single_instance_subscription() -> Subscription<Message> {
    SINGLE_INSTANCE
        .get()
        .filter(|instance| instance.supports_signals())
        .map(|_| Subscription::run(single_instance_signals))
        .unwrap_or_else(Subscription::none)
}

fn single_instance_signals() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(8, async |mut output| {
        let Some(instance) = SINGLE_INSTANCE.get() else {
            return;
        };

        std::thread::spawn(move || {
            loop {
                match instance.accept_signal() {
                    Ok(Signal::Show(request)) => {
                        if output
                            .try_send(Message::SingleInstanceShowRequested(request))
                            .is_err_and(|error| error.is_disconnected())
                        {
                            break;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_error) => break,
                }
            }
        });

        std::future::pending::<()>().await;
    })
}

#[cfg(test)]
mod tests;
