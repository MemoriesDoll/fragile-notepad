use iced::{Subscription, Task, event, keyboard, stream, window};

use crate::core::{DocumentId, EditorSettings, FindState, Workspace};
use crate::editor::{EditorAction, EditorSelection, OutlineParseResult, OutlineState};
use crate::ipc::{PrimaryInstance, Signal};
use crate::message::{AboutTab, Menu, Message, SaveRequest};
use crate::search_dialog::SearchDialogState;
use crate::settings_dialog::SettingsDialogState;
use crate::ui;

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use animation::ChromeAnimation;
use close_prompt::ClosePrompt;

use windowing::{AdvancedSearchWindow, SettingsWindow};

mod animation;
mod close_prompt;
mod editor;
mod editor_ops;
mod files;
mod go_to_line;
mod menu;
mod outline;
mod presentation;
mod rendering;
mod search;
mod session;
mod settings;
mod shortcuts;
mod syntax;
mod windowing;

static SINGLE_INSTANCE: OnceLock<PrimaryInstance> = OnceLock::new();

#[derive(Debug)]
pub struct App {
    workspace: Workspace,
    find: FindState,
    settings: EditorSettings,
    outline_parsing: outline::OutlineParsing,
    syntax_parsing: syntax::SyntaxParsing,
    is_loading: bool,
    pending_save: Option<SaveRequest>,
    pending_reloads: HashMap<DocumentId, crate::core::Document>,
    pending_save_all: VecDeque<crate::core::DocumentId>,
    pending_close_after_save: Option<crate::core::DocumentId>,
    pending_close_documents: VecDeque<crate::core::DocumentId>,
    close_prompt: ClosePrompt,
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
    go_to_line_prompt: Option<go_to_line::GoToLinePrompt>,
    dragged_tab: Option<crate::core::DocumentId>,
    hovered_drop_tab: Option<crate::core::DocumentId>,
    keyboard_modifiers: keyboard::Modifiers,
    focused_window_id: Option<window::Id>,
    maximized_windows: HashMap<window::Id, bool>,
    title_bar_style: ui::title_bar::ControlStyle,
    rendering: rendering::RenderingState,
    chrome_animation: ChromeAnimation,
    main_window_opened: bool,
    pending_startup_gpu_boost: bool,
    session: session::SessionState,
    settings_loaded: bool,
    settings_read_failed: bool,
    initial_settings_edits: u32,
    settings_dirty: bool,
    settings_flush_scheduled: bool,
    loading_find_scheduled: bool,
    analysis_in_flight: Option<(DocumentId, u64, String, usize)>,
    load_handles: HashMap<DocumentId, iced::task::Handle>,
    pending_search: Option<search::PendingSearch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseGoal {
    KeepOpen,
    ExitApp,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        Self::new_with_options(crate::startup::StartupOptions {
            files: Vec::new(),
            restore_session: false,
        })
    }

    pub fn new_with_options(options: crate::startup::StartupOptions) -> (Self, Task<Message>) {
        let (main_window_id, open) = window::open(windowing::custom_chrome(window::Settings {
            min_size: Some(iced::Size::new(640.0, 400.0)),
            exit_on_close_request: false,
            ..window::Settings::default()
        }));

        let mut app = Self {
            workspace: Workspace::new(),
            find: FindState::new(),
            settings: EditorSettings::default(),
            outline_parsing: outline::OutlineParsing::new(),
            syntax_parsing: syntax::SyntaxParsing::default(),
            is_loading: false,
            pending_save: None,
            pending_reloads: HashMap::new(),
            pending_save_all: VecDeque::new(),
            pending_close_after_save: None,
            pending_close_documents: VecDeque::new(),
            close_prompt: ClosePrompt::new(),
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
            go_to_line_prompt: None,
            dragged_tab: None,
            hovered_drop_tab: None,
            keyboard_modifiers: keyboard::Modifiers::default(),
            focused_window_id: Some(main_window_id),
            maximized_windows: HashMap::new(),
            title_bar_style: ui::title_bar::ControlStyle::startup(),
            rendering: rendering::RenderingState::Software,
            chrome_animation: ChromeAnimation::new(),
            main_window_opened: false,
            pending_startup_gpu_boost: false,
            session: session::SessionState::new(options),
            settings_loaded: false,
            settings_read_failed: false,
            initial_settings_edits: 0,
            settings_dirty: false,
            settings_flush_scheduled: false,
            loading_find_scheduled: false,
            analysis_in_flight: None,
            load_handles: HashMap::new(),
            pending_search: None,
        };

        app.refresh_find_matches();
        let outline_task = app.schedule_outline_parse(app.workspace.active_document_id);

        let session_task = if app.session.enabled {
            Task::perform(
                crate::services::session_store::load_session(),
                Message::SessionLoaded,
            )
        } else {
            Task::none()
        };
        (
            app,
            Task::batch([
                open.map(Message::WindowOpened),
                iced::widget::operation::focus(crate::ui::editor::EDITOR_ID),
                Task::perform(crate::services::load_settings(), Message::SettingsLoaded),
                outline_task,
                session_task,
            ]),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let track_session = self.session_should_track(&message);
        if self.session.exiting
            && !matches!(
                message,
                Message::ShutdownPersisted(_)
                    | Message::SyntaxParsed(..)
                    | Message::SettingsPersisted(_)
                    | Message::SessionPersisted(_)
            )
        {
            if let Message::ForwardedFiles(_, _, receipt) = &message {
                receipt.resolve(false);
            }
            return Task::none();
        }
        let task = self.update_traced(message);
        let search_task = if self.session.exiting {
            Task::none()
        } else {
            self.resume_pending_search()
        };
        let session_task = if track_session {
            self.request_session_save()
        } else {
            Task::none()
        };
        let analysis = self.schedule_active_analysis();
        let syntax = self.schedule_syntax_parse();
        Task::batch([task, search_task, session_task, analysis, syntax])
    }

    fn update_traced(&mut self, message: Message) -> Task<Message> {
        if let Some(perf_span) = crate::perf_trace::span("app_update", format_args!("{message:?}"))
        {
            let task = self.update_inner(message);
            perf_span.end_with("");

            return task;
        }

        self.update_inner(message)
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
        if !self.settings_loaded {
            self.initial_settings_edits |= session::settings_edit_mask(&message);
        }
        match message {
            #[cfg(debug_assertions)]
            Message::ToggleTitleBarStyle => {
                self.title_bar_style = self.title_bar_style.toggled();
                Task::none()
            }
            message @ (Message::WindowChrome(..) | Message::WindowMaximized(..)) => {
                self.update_window(message)
            }
            Message::SyntaxParsed(id, result) => self.complete_syntax_parse(id, result),
            Message::ForwardedFiles(paths, request, receipt) => {
                if !receipt.try_accept() {
                    return Task::none();
                }
                Task::batch([self.open_paths(paths), self.show_main_window(request)])
            }
            Message::OpenPaths(paths) => self.open_paths(paths),
            Message::StartupReady => {
                self.session.ready = true;
                self.restore_startup()
            }
            Message::StartupFrameReady => {
                crate::startup::report_first_frame_ready();
                Task::none()
            }
            Message::SessionLoaded(result) => self.session_loaded(result),
            Message::SessionFlush => self.flush_session(),
            Message::SessionPersisted(result) => self.session_persisted(result),
            Message::ShutdownPersisted(result) => self.shutdown_persisted(result),
            Message::SettingsFlush => self.flush_settings(),
            Message::RefreshLoadingFind => {
                self.loading_find_scheduled = false;
                self.refresh_find_matches();
                Task::none()
            }
            Message::DocumentAnalyzed(result) => {
                self.analysis_in_flight = None;
                if let Some(document) = self.workspace.document_mut(result.document_id) {
                    if document.apply_analysis(result)
                        && let Some(ranges) = self.session.folds.remove(&document.id)
                    {
                        document.restore_collapsed_folds(&ranges);
                    }
                }
                Task::none()
            }
            Message::None => Task::none(),
            Message::SingleInstanceShowRequested(request) => self.show_main_window(request),
            message @ (Message::GoToLineOpened
            | Message::GoToLineChanged(_)
            | Message::GoToLineSubmitted
            | Message::GoToLineClosed) => self.update_go_to_line(message),
            Message::Shortcut(shortcut) => self.update_shortcut(shortcut),
            Message::RuntimeEvent(event, status, window_id) => {
                self.update_runtime_event(event, status, window_id)
            }
            Message::EditorAction(document_id, action) => self.update_editor(document_id, action),
            Message::OutlineParseCompleted(result) => self.complete_outline_parse(result),
            Message::ClipboardRead(request, result) => self.update_clipboard_read(request, result),
            Message::ClipboardWritten(_result) => Task::none(),
            Message::BackendBoostRequested => self.request_gpu_boost(),
            Message::BackendBoostConfigured(result) => self.complete_gpu_boost(result),
            Message::ChromeAnimationFrame(at) => self.update_chrome_animation_frame(at),
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
                self.chrome_animation.about.set_visible(false);
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
            Message::Uppercase => self.update_active_editor_command(EditorAction::Uppercase),
            Message::Lowercase => self.update_active_editor_command(EditorAction::Lowercase),
            Message::TrimTrailingSpaces => {
                self.update_active_editor_command(EditorAction::TrimTrailingSpaces)
            }
            Message::JoinLines => self.update_active_editor_command(EditorAction::JoinLines),
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
            | Message::SaveCopyAs
            | Message::FileSaved(_, _)
            | Message::FileCopySaved(_, _)
            | Message::ReloadFromDisk
            | Message::EncodingSelected(_)
            | Message::CloseFile
            | Message::CloseAllFiles
            | Message::CloseAllButActiveFile
            | Message::CloseAllButPinnedFiles
            | Message::CloseAllToLeft
            | Message::CloseAllToRight
            | Message::CloseAllUnchanged
            | Message::DirtyCloseResolved(_, _)
            | Message::DirtyCloseFadeFinished(_)) => self.update_file(message),
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

                let startup_task = if self.main_window_id == Some(id)
                    && (self.session.enabled || !self.session.paths.is_empty())
                {
                    Task::perform(
                        async {
                            tokio::time::sleep(Duration::from_millis(16)).await;
                        },
                        |_| Message::StartupReady,
                    )
                } else {
                    Task::none()
                };
                let probe_task =
                    if self.main_window_id == Some(id) && crate::startup::startup_probe_enabled() {
                        window::screenshot(id).map(|_| Message::StartupFrameReady)
                    } else {
                        Task::none()
                    };
                let window_task = Task::batch([
                    self.update_window(Message::WindowOpened(id)),
                    startup_task,
                    probe_task,
                ]);

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

    pub fn subscription(&self) -> Subscription<Message> {
        let chrome_animation = if self.needs_animation_frames() {
            window::frames().map(Message::ChromeAnimationFrame)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            single_instance_subscription(),
            event::listen_with(shortcuts::event_to_message),
            window::close_requests().map(Message::WindowCloseRequested),
            window::close_events().map(Message::WindowClosed),
            chrome_animation,
        ])
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

    fn active_outline_state(&self) -> Option<&OutlineState> {
        self.workspace
            .active_document()
            .and_then(|document| self.outline_parsing.state_for(document))
    }

    fn schedule_outline_parse(&mut self, document_id: DocumentId) -> Task<Message> {
        if document_id != self.workspace.active_document_id {
            return Task::none();
        }
        let Some(document) = self.workspace.document(document_id) else {
            self.outline_parsing.remove(document_id);
            return Task::none();
        };
        self.outline_parsing
            .schedule(document)
            .map(Message::OutlineParseCompleted)
    }

    fn complete_outline_parse(&mut self, result: OutlineParseResult) -> Task<Message> {
        self.outline_parsing
            .complete(self.workspace.document(result.document_id), result);
        Task::none()
    }

    fn update_active_fold_command(&mut self, action: EditorAction) -> Task<Message> {
        self.update_active_editor_command(action)
    }

    fn update_active_editor_command(&mut self, action: EditorAction) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();

        Task::batch([
            self.update_editor(self.workspace.active_document_id, action),
            iced::widget::operation::focus(crate::ui::editor::EDITOR_ID),
        ])
    }

    fn toggle_function_list(&mut self) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();
        self.is_function_list_visible = !self.is_function_list_visible;
        self.chrome_animation
            .function_list
            .set_visible(self.is_function_list_visible);

        Task::none()
    }

    fn open_about_dialog(&mut self) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();
        self.is_about_visible = true;
        if !self.chrome_animation.about.rendered_visible() {
            self.about_tab = AboutTab::About;
        }
        self.chrome_animation.about.set_visible(true);

        self.request_about_gpu_boost()
    }

    fn needs_animation_frames(&self) -> bool {
        self.chrome_animation.needs_frames()
            || self.close_prompt.needs_frames()
            || self
                .go_to_line_prompt
                .as_ref()
                .is_some_and(|prompt| prompt.animation.needs_frames())
    }

    fn update_chrome_animation_frame(&mut self, at: Instant) -> Task<Message> {
        self.chrome_animation.update_frame(at);
        let close = self
            .close_prompt
            .update_frame(at)
            .map(Message::DirtyCloseFadeFinished)
            .map_or_else(Task::none, Task::done);
        Task::batch([close, self.update_go_to_line_frame(at)])
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
        document.set_main_selection(EditorSelection::new(position, position));
        document.preferred_vertical_column = None;
        document.reveal_position(position);

        Task::none()
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
                let mut disconnected = false;
                let accepted = instance.accept_signal_with(|signal| {
                    use iced::futures::SinkExt;
                    let receipt = crate::ipc::AdmissionReceipt::new();
                    let (paths, request) = match signal {
                        Signal::Show(request) => (Vec::new(), request.clone()),
                        Signal::OpenFiles(paths, request) => (paths.clone(), request.clone()),
                    };
                    if futures::executor::block_on(output.send(Message::ForwardedFiles(
                        paths,
                        request,
                        receipt.clone(),
                    )))
                    .is_err()
                    {
                        disconnected = true;
                        return false;
                    }
                    receipt.wait_for_acceptance()
                });
                if disconnected {
                    break;
                }
                match accepted {
                    Ok(_) => {}
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
