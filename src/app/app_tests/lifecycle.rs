use super::test_support::*;
use iced::backend;

fn strict_handoff_result(
    completed_phase: backend::StrictHandoffPhase,
    rollback: backend::StrictRollbackStatus,
) -> backend::StrictHandoffOutcome {
    Ok(backend::StrictHandoffResult {
        completed_phase,
        rollback,
        windows: Vec::new(),
    })
}

fn strict_handoff_error(
    category: backend::StrictHandoffFailureCategory,
) -> backend::StrictHandoffOutcome {
    Err(backend::StrictHandoffError {
        phase: match category {
            backend::StrictHandoffFailureCategory::WarmUp => backend::StrictHandoffPhase::Warming,
            backend::StrictHandoffFailureCategory::Commit => {
                backend::StrictHandoffPhase::CommitPending
            }
            backend::StrictHandoffFailureCategory::FirstPresent
            | backend::StrictHandoffFailureCategory::RendererEvidenceMissing => {
                backend::StrictHandoffPhase::AwaitingFirstPresent
            }
            _ => backend::StrictHandoffPhase::Preparing,
        },
        category,
        rollback: backend::StrictRollbackStatus::Restored,
        windows: Vec::new(),
        message: format!("{category:?} injected failure"),
    })
}

fn strict_handoff_success() -> backend::StrictHandoffOutcome {
    strict_handoff_result(
        backend::StrictHandoffPhase::Completed,
        backend::StrictRollbackStatus::ReleasedAfterSuccess,
    )
}

fn render_backend_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct RenderBackendEnvGuard {
    previous: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl RenderBackendEnvGuard {
    fn new(value: Option<&str>) -> Self {
        let lock = render_backend_env_lock()
            .lock()
            .expect("render backend env lock should not be poisoned");
        let previous = std::env::var(crate::app::rendering::RENDER_BACKEND_ENV).ok();

        unsafe {
            match value {
                Some(value) => std::env::set_var(crate::app::rendering::RENDER_BACKEND_ENV, value),
                None => std::env::remove_var(crate::app::rendering::RENDER_BACKEND_ENV),
            }
        }

        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for RenderBackendEnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(crate::app::rendering::RENDER_BACKEND_ENV, value),
                None => std::env::remove_var(crate::app::rendering::RENDER_BACKEND_ENV),
            }
        }
    }
}

#[test]
fn default_startup_stays_software_first_until_loaded_settings_request_boost() {
    let (mut app, _) = App::new();
    let main_window = app
        .main_window_id
        .expect("main window should be tracked after boot");

    assert_eq!(
        app.settings.hardware_acceleration,
        HardwareAccelerationMode::Lazy
    );
    assert!(!app.pending_startup_gpu_boost);

    let task = app.update(Message::WindowOpened(main_window));

    assert_eq!(task.units(), 0);
    assert!(!app.pending_startup_gpu_boost);
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Software
    );
}

#[test]
fn secondary_window_open_does_not_consume_startup_gpu_boost() {
    let (mut app, _) = App::new();
    let secondary_window = iced::window::Id::unique();

    app.pending_startup_gpu_boost = true;
    let task = app.update(Message::WindowOpened(secondary_window));

    assert_eq!(task.units(), 0);
    assert!(app.pending_startup_gpu_boost);
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Software
    );
}

#[test]
fn main_window_open_consumes_startup_gpu_boost() {
    let (mut app, _) = App::new();
    let main_window = app
        .main_window_id
        .expect("main window should be tracked after boot");

    app.settings.hardware_acceleration = HardwareAccelerationMode::Lazy;
    // Simulates an explicit startup boost queued after persisted settings load
    // or a diagnostic override, not the App::new default.
    app.pending_startup_gpu_boost = true;
    let _ = app.update(Message::WindowOpened(main_window));

    assert!(!app.pending_startup_gpu_boost);
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::PreparingHardware
    );
}

#[test]
fn manual_gpu_boost_request_still_starts_immediately() {
    let (mut app, _) = App::new();

    app.settings.hardware_acceleration = HardwareAccelerationMode::Lazy;
    let _ = app.update(Message::BackendBoostRequested);

    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::PreparingHardware
    );
}

#[test]
fn applying_lazy_rendering_setting_requests_runtime_gpu_boost() {
    let _env = RenderBackendEnvGuard::new(None);
    let (mut app, _) = App::new();

    app.settings.hardware_acceleration = HardwareAccelerationMode::Off;
    app.settings_dialog.reset_from(&app.settings);
    let _ = app.update(Message::DraftHardwareAccelerationSelected(
        HardwareAccelerationMode::Lazy,
    ));
    let _ = app.update(Message::ApplySettings);

    assert_eq!(
        app.settings.hardware_acceleration,
        HardwareAccelerationMode::Lazy
    );
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::PreparingHardware
    );
}

#[test]
fn saving_diagnostic_rendering_setting_requests_runtime_gpu_boost() {
    let _env = RenderBackendEnvGuard::new(None);
    let (mut app, _) = App::new();

    let _ = app.update(Message::DraftHardwareAccelerationSelected(
        HardwareAccelerationMode::Diagnostic,
    ));
    let _ = app.update(Message::SaveSettings);

    assert_eq!(
        app.settings.hardware_acceleration,
        HardwareAccelerationMode::Diagnostic
    );
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::PreparingHardware
    );
}

#[test]
fn software_rendering_backend_env_suppresses_applied_hardware_boost() {
    let _env = RenderBackendEnvGuard::new(Some("software"));
    let (mut app, _) = App::new();

    let _ = app.update(Message::DraftHardwareAccelerationSelected(
        HardwareAccelerationMode::Diagnostic,
    ));
    let _ = app.update(Message::ApplySettings);

    assert_eq!(
        app.settings.hardware_acceleration,
        HardwareAccelerationMode::Diagnostic
    );
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Software
    );
}

#[test]
fn strict_completed_released_handoff_enters_hardware_state() {
    let (mut app, _) = App::new();

    app.rendering = crate::app::rendering::RenderingState::PreparingHardware;
    let _ = app.update(Message::BackendBoostConfigured(strict_handoff_success()));

    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Hardware
    );
}

#[test]
fn strict_ok_result_fails_closed_without_completed_released_success() {
    let cases = [
        (
            backend::StrictHandoffPhase::AwaitingFirstPresent,
            backend::StrictRollbackStatus::ReleasedAfterSuccess,
        ),
        (
            backend::StrictHandoffPhase::Completed,
            backend::StrictRollbackStatus::NotNeeded,
        ),
    ];

    for (completed_phase, rollback) in cases {
        let (mut app, _) = App::new();
        app.rendering = crate::app::rendering::RenderingState::PreparingHardware;

        let _ = app.update(Message::BackendBoostConfigured(strict_handoff_result(
            completed_phase,
            rollback,
        )));

        assert_eq!(
            app.rendering,
            crate::app::rendering::RenderingState::Failed(
                crate::app::rendering::RenderFailureCategory::Unknown
            ),
            "phase {completed_phase:?} rollback {rollback:?} should fail closed"
        );
    }
}

#[test]
fn strict_handoff_error_categories_map_to_render_failures() {
    let cases = [
        (
            backend::StrictHandoffFailureCategory::Prepare,
            crate::app::rendering::RenderFailureCategory::Prepare,
        ),
        (
            backend::StrictHandoffFailureCategory::WarmUp,
            crate::app::rendering::RenderFailureCategory::WarmUp,
        ),
        (
            backend::StrictHandoffFailureCategory::Commit,
            crate::app::rendering::RenderFailureCategory::Commit,
        ),
        (
            backend::StrictHandoffFailureCategory::FirstPresent,
            crate::app::rendering::RenderFailureCategory::FirstPresent,
        ),
        (
            backend::StrictHandoffFailureCategory::AlreadyInProgress,
            crate::app::rendering::RenderFailureCategory::AlreadyInProgress,
        ),
        (
            backend::StrictHandoffFailureCategory::NoActiveWindow,
            crate::app::rendering::RenderFailureCategory::NoActiveWindow,
        ),
        (
            backend::StrictHandoffFailureCategory::Cancelled,
            crate::app::rendering::RenderFailureCategory::Cancelled,
        ),
        (
            backend::StrictHandoffFailureCategory::Unsupported,
            crate::app::rendering::RenderFailureCategory::Unsupported,
        ),
        (
            backend::StrictHandoffFailureCategory::RendererEvidenceMissing,
            crate::app::rendering::RenderFailureCategory::RendererEvidenceMissing,
        ),
        (
            backend::StrictHandoffFailureCategory::RollbackFailed,
            crate::app::rendering::RenderFailureCategory::Rollback,
        ),
    ];

    for (strict_category, expected_category) in cases {
        let (mut app, _) = App::new();
        app.rendering = crate::app::rendering::RenderingState::PreparingHardware;

        let _ = app.update(Message::BackendBoostConfigured(strict_handoff_error(
            strict_category,
        )));

        assert_eq!(
            app.rendering,
            crate::app::rendering::RenderingState::Failed(expected_category),
            "strict category {strict_category:?} should map structurally"
        );
    }
}

#[test]
fn settings_window_close_request_discards_draft_and_tracks_closed_event() {
    let (mut app, _) = App::new();
    let original_word_wrap = app.settings.word_wrap;

    let _ = app.update(Message::ToggleSettingsPanel);
    let settings_window = app
        .settings_window
        .expect("settings window should be tracked")
        .id();
    let _ = app.update(Message::DraftWordWrapToggled(!original_word_wrap));

    assert!(
        app.settings_window
            .is_some_and(|window| window.is(settings_window))
    );
    assert_eq!(app.settings_dialog.draft.word_wrap, !original_word_wrap);

    let _ = app.update(Message::WindowCloseRequested(settings_window));

    assert!(
        app.settings_window
            .is_some_and(|window| window.is(settings_window))
    );
    assert_eq!(app.settings_dialog.draft.word_wrap, original_word_wrap);

    let _ = app.update(Message::WindowClosed(settings_window));

    assert!(app.settings_window.is_none());
    assert_eq!(app.settings_dialog.draft.word_wrap, original_word_wrap);
}

#[test]
fn advanced_search_window_close_tracks_native_lifecycle_and_reopens_cleanly() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::FindQueryChanged("needle".to_owned()));
    let _ = app.update(Message::ToggleAdvancedSearch(
        crate::message::AdvancedSearchTab::Find,
    ));
    let search_window = app
        .advanced_search_window
        .expect("advanced search window should be tracked")
        .id();

    assert_eq!(app.search_dialog.query, "needle");
    assert_eq!(
        app.search_dialog.active_tab,
        crate::message::AdvancedSearchTab::Find
    );

    let _ = app.update(Message::AdvancedSearchQueryChanged("draft".to_owned()));
    let _ = app.update(Message::WindowCloseRequested(search_window));

    assert!(
        app.advanced_search_window
            .is_some_and(|window| window.is(search_window))
    );
    assert_eq!(app.search_dialog.query, "draft");

    let _ = app.update(Message::WindowClosed(search_window));

    assert!(app.advanced_search_window.is_none());
    assert_eq!(app.search_dialog.query, "draft");

    let _ = app.update(Message::ToggleAdvancedSearch(
        crate::message::AdvancedSearchTab::Replace,
    ));
    let reopened_window = app
        .advanced_search_window
        .expect("advanced search window should reopen")
        .id();

    assert_ne!(reopened_window, search_window);
    assert_eq!(
        app.search_dialog.active_tab,
        crate::message::AdvancedSearchTab::Replace
    );
    assert_eq!(
        app.search_dialog.query, app.find.query,
        "reopening advanced search should sync from the inline find model"
    );
}

#[test]
fn toggling_advanced_search_while_open_reuses_window_and_updates_tab() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::ToggleAdvancedSearch(
        crate::message::AdvancedSearchTab::Find,
    ));
    let search_window = app
        .advanced_search_window
        .expect("advanced search window should be tracked")
        .id();

    let _ = app.update(Message::ToggleAdvancedSearch(
        crate::message::AdvancedSearchTab::FindInFiles,
    ));

    assert!(
        app.advanced_search_window
            .is_some_and(|window| window.is(search_window))
    );
    assert_eq!(
        app.search_dialog.active_tab,
        crate::message::AdvancedSearchTab::FindInFiles
    );
}

#[test]
fn close_clean_exits() {
    let (mut app, _) = App::new();
    let main_window = app
        .main_window_id
        .expect("main window should be tracked after boot");

    let task = app.update(Message::WindowCloseRequested(main_window));

    assert_eq!(task.units(), 1);
}

#[test]
fn close_dirty_prompts() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let main_window = app
        .main_window_id
        .expect("main window should be tracked after boot");

    app.workspace
        .active_document_mut()
        .expect("active document")
        .mark_dirty();

    let task = app.update(Message::WindowCloseRequested(main_window));

    assert_eq!(task.units(), 0);
    assert_eq!(app.pending_dirty_close, Some(document_id));
    assert_eq!(app.close_goal, crate::app::CloseGoal::ExitApp);
    assert!(app.workspace.document(document_id).is_some());
}

#[test]
fn cancel_exit_keeps_app_open() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let main_window = app
        .main_window_id
        .expect("main window should be tracked after boot");

    app.workspace
        .active_document_mut()
        .expect("active document")
        .mark_dirty();

    let _ = app.update(Message::WindowCloseRequested(main_window));
    let task = app.update(Message::DirtyCloseResolved(
        document_id,
        DirtyCloseDecision::Cancel,
    ));

    assert_eq!(task.units(), 0);
    assert_eq!(app.close_goal, CloseGoal::KeepOpen);
    assert!(app.workspace.document(document_id).is_some());
}

#[test]
fn save_cancel_keeps_app_open() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let main_window = app
        .main_window_id
        .expect("main window should be tracked after boot");

    app.workspace
        .active_document_mut()
        .expect("active document")
        .mark_dirty();

    let _ = app.update(Message::WindowCloseRequested(main_window));
    let _ = app.update(Message::DirtyCloseResolved(
        document_id,
        DirtyCloseDecision::Save,
    ));
    let request = app
        .pending_save
        .clone()
        .expect("dirty close save should start a save");
    let task = app.update(Message::FileSaved(
        request,
        Err(crate::message::FileError::DialogClosed),
    ));

    assert_eq!(task.units(), 0);
    assert_eq!(app.close_goal, CloseGoal::KeepOpen);
    assert!(app.workspace.document(document_id).is_some());
}

#[test]
fn settings_persist_error_sets_visible_status() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::SettingsPersisted(Err(
        crate::message::SettingsError::Io(std::io::ErrorKind::PermissionDenied),
    )));

    assert_eq!(
        app.file_status.as_deref(),
        Some("Settings save failed: I/O error")
    );
}

#[test]
fn about_dialog_opens_switches_tabs_and_closes() {
    let (mut app, _) = App::new();

    app.active_menu = Some(Menu::Help);
    app.active_menu_path = vec![String::from("about")];
    let _ = app.update(Message::AboutOpened);

    assert!(app.is_about_visible);
    assert_eq!(app.about_tab, AboutTab::About);
    assert_eq!(app.active_menu, None);
    assert!(app.active_menu_path.is_empty());

    let _ = app.update(Message::AboutTabSelected(AboutTab::Licenses));
    assert_eq!(app.about_tab, AboutTab::Licenses);

    let _ = app.update(Message::AboutClosed);
    assert!(!app.is_about_visible);
    assert_eq!(app.about_animation, crate::app::AboutOverlayAnimation::Idle);
}

#[test]
fn about_open_waits_for_hardware_before_animation_starts() {
    let _env = RenderBackendEnvGuard::new(None);
    let (mut app, _) = App::new();

    app.settings.hardware_acceleration = HardwareAccelerationMode::Lazy;
    let _boost_request_task = app.update(Message::AboutOpened);

    assert!(app.is_about_visible);
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::WaitingForHardware {
            boost_requested: true
        }
    );
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Software
    );

    // Iced's public Task API does not expose the zero-unit Task::done output for
    // synchronous test inspection, so this simulates runtime delivery of it.
    let _ = app.update(Message::BackendBoostRequested);

    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::PreparingHardware
    );
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::WaitingForHardware {
            boost_requested: true
        }
    );

    let waiting_info = app.about_animation_info();
    assert_eq!(waiting_info.progress, 0.0);
    assert_eq!(waiting_info.visual_progress, 0.0);
    assert!(!waiting_info.is_animating);
}

#[test]
fn about_waiting_animation_ignores_frame_ticks_until_hardware_finishes() {
    let _env = RenderBackendEnvGuard::new(None);
    let (mut app, _) = App::new();

    app.settings.hardware_acceleration = HardwareAccelerationMode::Lazy;
    let _ = app.update(Message::AboutOpened);
    let _ = app.update(Message::BackendBoostRequested);
    let frame = std::time::Instant::now();

    let _ = app.update(Message::AboutAnimationFrame(frame));
    let _ = app.update(Message::AboutAnimationFrame(
        frame + std::time::Duration::from_secs(1),
    ));

    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::WaitingForHardware {
            boost_requested: true
        }
    );

    let waiting_info = app.about_animation_info();
    assert_eq!(waiting_info.progress, 0.0);
    assert_eq!(waiting_info.visual_progress, 0.0);
    assert!(!waiting_info.is_animating);
}

#[test]
fn about_open_does_not_duplicate_existing_hardware_prepare() {
    let (mut app, _) = App::new();

    app.rendering = crate::app::rendering::RenderingState::PreparingHardware;
    let task = app.update(Message::AboutOpened);

    assert!(app.is_about_visible);
    assert_eq!(task.units(), 0);
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::WaitingForHardware {
            boost_requested: false
        }
    );
}

#[test]
fn about_animation_starts_only_after_successful_hardware_completion() {
    let (mut app, _) = App::new();

    app.rendering = crate::app::rendering::RenderingState::PreparingHardware;
    let _ = app.update(Message::AboutOpened);
    let _ = app.update(Message::BackendBoostConfigured(strict_handoff_success()));

    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Hardware
    );
    assert!(matches!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::Running {
            started_at: None,
            progress: 0.0
        }
    ));

    let first_frame = std::time::Instant::now();
    let _ = app.update(Message::AboutAnimationFrame(first_frame));

    assert!(matches!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::Running {
            started_at: Some(_),
            progress: 0.0
        }
    ));
}

#[test]
fn about_animation_is_disabled_on_boost_failure() {
    let (mut app, _) = App::new();

    app.rendering = crate::app::rendering::RenderingState::PreparingHardware;
    let _ = app.update(Message::AboutOpened);
    let _ = app.update(Message::BackendBoostConfigured(strict_handoff_error(
        backend::StrictHandoffFailureCategory::Prepare,
    )));

    assert!(app.is_about_visible);
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Failed(
            crate::app::rendering::RenderFailureCategory::Prepare
        )
    );
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::Disabled
    );
}

#[test]
fn about_dialog_remains_usable_after_boost_failure() {
    let (mut app, _) = App::new();

    app.rendering = crate::app::rendering::RenderingState::PreparingHardware;
    let _ = app.update(Message::AboutOpened);
    let _ = app.update(Message::BackendBoostConfigured(strict_handoff_error(
        backend::StrictHandoffFailureCategory::RendererEvidenceMissing,
    )));

    assert!(app.is_about_visible);
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Failed(
            crate::app::rendering::RenderFailureCategory::RendererEvidenceMissing
        )
    );
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::Disabled
    );
    assert!(
        app.file_status
            .as_deref()
            .is_some_and(|status| status.contains("renderer-evidence"))
    );

    let _ = app.update(Message::AboutTabSelected(AboutTab::Debug));
    assert_eq!(app.about_tab, AboutTab::Debug);

    let _ = app.update(Message::AboutClosed);
    assert!(!app.is_about_visible);
    assert_eq!(app.about_animation, crate::app::AboutOverlayAnimation::Idle);

    let _ = app.update(Message::AboutOpened);
    assert!(app.is_about_visible);
    assert_eq!(app.about_tab, AboutTab::About);
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::Disabled
    );
}

#[test]
fn about_animation_is_disabled_when_software_policy_forces_fallback() {
    let _env = RenderBackendEnvGuard::new(Some("software"));
    let (mut app, _) = App::new();

    let task = app.update(Message::AboutOpened);

    assert!(app.is_about_visible);
    assert_eq!(task.units(), 0);
    assert_eq!(
        app.rendering,
        crate::app::rendering::RenderingState::Software
    );
    assert_eq!(
        app.about_animation,
        crate::app::AboutOverlayAnimation::Disabled
    );
}

#[test]
fn about_close_resets_waiting_animation_state() {
    let (mut app, _) = App::new();

    app.rendering = crate::app::rendering::RenderingState::PreparingHardware;
    let _ = app.update(Message::AboutOpened);
    let _ = app.update(Message::AboutClosed);

    assert!(!app.is_about_visible);
    assert_eq!(app.about_animation, crate::app::AboutOverlayAnimation::Idle);
}

#[test]
fn language_selection_updates_active_document_without_dirtying_it() {
    let (mut app, _) = App::new();

    app.active_menu = Some(Menu::Language);
    let _ = app.update(Message::LanguageSelected("rs".to_owned()));

    let document = app
        .workspace
        .active_document()
        .expect("workspace should have an active document");

    assert_eq!(document.syntax_token, "rs");
    assert!(!document.is_dirty);
    assert_eq!(app.active_menu, None);
}

#[test]
fn menu_path_tracks_generic_flyout_state_and_resets_on_menu_change() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::MenuToggled(Menu::Encoding));
    let _ = app.update(Message::MenuPathHovered(crate::message::MenuPath {
        depth: 0,
        segments: vec!["character-sets".to_owned()],
    }));

    assert_eq!(app.active_menu, Some(Menu::Encoding));
    assert_eq!(app.active_menu_path, vec!["character-sets"]);

    let _ = app.update(Message::MenuHovered(Menu::File));

    assert_eq!(app.active_menu, Some(Menu::File));
    assert!(app.active_menu_path.is_empty());
}

#[test]
fn menu_path_can_collapse_to_parent_and_ignores_closed_menu_hover() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::MenuToggled(Menu::Encoding));
    let _ = app.update(Message::MenuPathHovered(crate::message::MenuPath {
        depth: 1,
        segments: vec!["character-sets".to_owned(), "western-european".to_owned()],
    }));

    assert_eq!(
        app.active_menu_path,
        vec!["character-sets", "western-european"]
    );

    let _ = app.update(Message::MenuPathHovered(crate::message::MenuPath {
        depth: 1,
        segments: vec!["character-sets".to_owned()],
    }));

    assert_eq!(app.active_menu_path, vec!["character-sets"]);

    let _ = app.update(Message::MenuClosed);
    let _ = app.update(Message::MenuPathHovered(crate::message::MenuPath {
        depth: 0,
        segments: vec!["character-sets".to_owned()],
    }));

    assert_eq!(app.active_menu, None);
    assert!(app.active_menu_path.is_empty());
}

#[test]
fn opening_rust_file_prewarms_visible_syntax_cache() {
    let (mut app, _) = App::new();
    let contents = (0..200)
        .map(|line| format!("pub fn function_{line}() -> usize {{ {line} }}"))
        .collect::<Vec<_>>()
        .join("\n");

    let _ = app.update(Message::FileOpened(Ok(OpenedFile {
        path: PathBuf::from("widget.rs"),
        contents: Arc::new(crate::core::DecodedText {
            text: contents,
            encoding: crate::core::TextEncoding::Utf8,
            had_errors: false,
        }),
    })));

    let document = app.workspace.active_document().expect("active document");

    assert_eq!(document.syntax_token, "rs");
    assert!(
        document.syntax_cache.borrow().cached_line_count() >= SYNTAX_PREWARM_VISIBLE_LINES,
        "opening a highlighted file should prepare the initial syntax window before first draw"
    );
}

#[test]
fn undo_and_redo_commands_update_active_document() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    let _ = app.update(Message::EditorAction(
        document_id,
        EditorAction::InsertText("x".to_owned()),
    ));

    assert_eq!(app.workspace.active_document().expect("active").text(), "x");
    assert!(app.workspace.active_document().expect("active").is_dirty);

    let _ = app.update(Message::Undo);

    assert_eq!(app.workspace.active_document().expect("active").text(), "");
    assert!(!app.workspace.active_document().expect("active").is_dirty);

    let _ = app.update(Message::Redo);

    assert_eq!(app.workspace.active_document().expect("active").text(), "x");
    assert!(app.workspace.active_document().expect("active").is_dirty);
}
