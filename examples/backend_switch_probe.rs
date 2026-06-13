#[cfg(feature = "hybrid-rendering")]
fn main() -> iced::Result {
    probe::run()
}

#[cfg(not(feature = "hybrid-rendering"))]
fn main() {
    println!(
        "BACKEND_SWITCH_PROBE_SKIPPED reason=requires_feature feature=hybrid-rendering message=\"run with --features hybrid-rendering\""
    );
}

#[cfg(feature = "hybrid-rendering")]
mod probe {
    use iced::backend::{self, Api};
    use iced::widget::{center, column, text};
    use iced::window;
    use iced::{Backend, Element, Length, Settings, Size, Subscription, Task, Theme};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const INITIAL_FRAME_TIMEOUT: Duration = Duration::from_secs(10);
    const STRICT_SWITCH_TIMEOUT: Duration = Duration::from_secs(30);
    const POST_SWITCH_FRAME_TIMEOUT: Duration = Duration::from_secs(10);
    const MODE_ENV: &str = "FRAGILE_BACKEND_SWITCH_PROBE_MODE";
    const FAILURE_ENV: &str = "FRAGILE_BACKEND_SWITCH_PROBE_FAILURE";
    const SCENARIO_ENV: &str = "FRAGILE_BACKEND_SWITCH_PROBE_SCENARIO";
    const RESULT_DIR_ENV: &str = "FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR";
    const RUNTIME_FAILURE_ENV: &str = "FRAGILE_NOTEPAD_RENDER_INJECT_FAILURE";
    const RUNTIME_PREPARE_DELAY_ENV: &str = "FRAGILE_NOTEPAD_RENDER_PREPARE_DELAY_MS";
    const RUNTIME_COMMIT_PENDING_DELAY_ENV: &str = "FRAGILE_NOTEPAD_RENDER_COMMIT_PENDING_DELAY_MS";
    const RUNTIME_COMMIT_PENDING_CLOSE_ENV: &str =
        "FRAGILE_NOTEPAD_RENDER_CLOSE_DURING_COMMIT_PENDING";
    const RUNTIME_TRACE_ENV: &str = "FRAGILE_PERF_TRACE";
    const RUNTIME_TRACE_DIR_ENV: &str = "FRAGILE_PERF_TRACE_DIR";
    const CARGO_TARGET_DIR_ENV: &str = "CARGO_TARGET_DIR";
    const PREPARE_DELAY_MS: &str = "250";
    const COMMIT_PENDING_DELAY_MS: &str = "250";

    pub fn run() -> iced::Result {
        let mode = ProbeMode::from_env_and_args();
        let failure = ProbeFailureMode::from_env_and_args();
        let scenario = ProbeScenario::from_env_and_args();
        scenario.install_runtime_env();
        failure.install_runtime_env();
        let trace_path = TraceValidation::install_for_probe(mode, scenario, failure);
        if let Some(trace_path) = &trace_path {
            trace_path.remove_existing();
        }
        let result_path = result_path(mode, scenario, failure);

        println!(
            "BACKEND_SWITCH_PROBE_START initial_backend=software mode={} failure={} scenario={}",
            mode.marker_value(),
            failure.marker_value(),
            scenario.marker_value()
        );
        println!(
            "BACKEND_SWITCH_PROBE_SCENARIO_{} mode={} failure={}",
            scenario.stable_suffix(),
            mode.marker_value(),
            failure.marker_value()
        );

        let result = iced::daemon(
            move || {
                Probe::new(
                    mode,
                    scenario,
                    failure,
                    trace_path.clone(),
                    result_path.clone(),
                )
            },
            Probe::update,
            Probe::view,
        )
        .subscription(Probe::subscription)
        .title(Probe::title)
        .theme(Theme::Light)
        .settings(Settings {
            backend: Backend::Software,
            antialiasing: false,
            vsync: true,
            ..Settings::default()
        })
        .run();

        result
    }

    struct Probe {
        mode: ProbeMode,
        scenario: ProbeScenario,
        failure: ProbeFailureMode,
        state: ProbeState,
        started_at: Instant,
        configured_at: Option<Instant>,
        switch_started_at: Option<Instant>,
        finished_from_state: Option<ProbeState>,
        frames_seen: u64,
        windows: Vec<WindowRecord>,
        window_open_requests: u64,
        windows_opened: u64,
        windows_closed: u64,
        resizes_seen: u64,
        resize_requested: bool,
        close_requested: bool,
        switch_started: bool,
        result: ProbeResult,
        reason: String,
        strict_outcome: Option<backend::StrictHandoffOutcome>,
        trace_path: Option<TraceValidation>,
        result_path: PathBuf,
        result_written: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeMode {
        PrepareWarmCommit,
        Configure,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeFailureMode {
        None,
        Prepare,
        Warm,
        Commit,
        FirstPresent,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeState {
        WaitingInitialFrame,
        Switching,
        WaitingPostSwitchFrame,
        Done,
    }

    #[derive(Debug, Clone)]
    enum Message {
        WindowOpened(window::Id),
        WindowEvent(window::Id, window::Event),
        Frame(Instant),
        BackendConfigured(Result<(), backend::Error>),
        StrictHandoffCompleted(backend::StrictHandoffOutcome),
        Tick(Instant),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeScenario {
        SingleWindow,
        MultiWindow,
        ResizeDuringPreparing,
        CloseDuringPreparing,
        CloseDuringCommitPending,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeResult {
        Ok,
        Failed,
        Indeterminate,
    }

    struct WindowRecord {
        id: window::Id,
        live: bool,
        post_switch_frame_seen: bool,
    }

    impl Probe {
        fn new(
            mode: ProbeMode,
            scenario: ProbeScenario,
            failure: ProbeFailureMode,
            trace_path: Option<TraceValidation>,
            result_path: PathBuf,
        ) -> (Self, Task<Message>) {
            let (main_window, open_main) = window::open(window::Settings {
                size: Size::new(420.0, 180.0),
                ..window::Settings::default()
            });

            (
                Self {
                    mode,
                    scenario,
                    failure,
                    state: ProbeState::WaitingInitialFrame,
                    started_at: Instant::now(),
                    configured_at: None,
                    switch_started_at: None,
                    finished_from_state: None,
                    frames_seen: 0,
                    windows: vec![WindowRecord {
                        id: main_window,
                        live: false,
                        post_switch_frame_seen: false,
                    }],
                    window_open_requests: 1,
                    windows_opened: 0,
                    windows_closed: 0,
                    resizes_seen: 0,
                    resize_requested: false,
                    close_requested: false,
                    switch_started: false,
                    result: ProbeResult::Failed,
                    reason: String::from("probe exited before recording a result"),
                    strict_outcome: None,
                    trace_path,
                    result_path,
                    result_written: false,
                },
                open_main.map(Message::WindowOpened),
            )
        }

        fn update(&mut self, message: Message) -> Task<Message> {
            match message {
                Message::WindowOpened(id) => {
                    self.windows_opened += 1;
                    self.windows
                        .iter_mut()
                        .find(|record| record.id == id)
                        .map(|record| record.live = true)
                        .unwrap_or_else(|| {
                            self.windows.push(WindowRecord {
                                id,
                                live: true,
                                post_switch_frame_seen: false,
                            });
                        });
                    println!(
                        "BACKEND_SWITCH_PROBE_WINDOW_OPENED id={id:?} count={}",
                        self.windows_opened
                    );
                    Task::none()
                }
                Message::WindowEvent(id, event) => self.window_event(id, event),
                Message::BackendConfigured(result) => match result {
                    Ok(()) => {
                        self.state = ProbeState::WaitingPostSwitchFrame;
                        self.configured_at = Some(Instant::now());
                        println!(
                            "BACKEND_SWITCH_PROBE_ANIMATION_BARRIER_OPEN target_backend=hardware note=\"animation consumers may start after backend configuration succeeds; this marker does not prove strict seamlessness\""
                        );
                        match self.mode {
                            ProbeMode::PrepareWarmCommit => println!(
                                "BACKEND_SWITCH_PROBE_HANDOFF_COMMIT_REQUESTED target_backend=hardware api=best mode={}",
                                self.mode.marker_value()
                            ),
                            ProbeMode::Configure => println!(
                                "BACKEND_SWITCH_PROBE_CONFIGURED target_backend=hardware api=best mode={}",
                                self.mode.marker_value()
                            ),
                        }
                        Task::none()
                    }
                    Err(error) => {
                        if self.failure.matches_error(&error) {
                            println!(
                                "BACKEND_SWITCH_PROBE_INJECTED_FAILURE_OBSERVED mode={} failure={} error={error:?}",
                                self.mode.marker_value(),
                                self.failure.marker_value()
                            );
                            self.finish(
                                ProbeResult::Ok,
                                format!(
                                    "injected {} failure observed",
                                    self.failure.marker_value()
                                ),
                            )
                        } else {
                            println!(
                                "BACKEND_SWITCH_PROBE_SWITCH_FAILED mode={} failure={} error={error:?}",
                                self.mode.marker_value(),
                                self.failure.marker_value()
                            );
                            self.finish(
                                ProbeResult::Failed,
                                format!("backend switch failed: {error:?}"),
                            )
                        }
                    }
                },
                Message::Frame(at) => self.frame(None, at),
                Message::StrictHandoffCompleted(outcome) => self.strict_handoff_completed(outcome),
                Message::Tick(now) => self.check_timeout(now),
            }
        }

        fn window_event(&mut self, id: window::Id, event: window::Event) -> Task<Message> {
            match event {
                window::Event::Closed => {
                    self.windows_closed += 1;
                    if let Some(record) = self.windows.iter_mut().find(|record| record.id == id) {
                        record.live = false;
                    }
                    println!(
                        "BACKEND_SWITCH_PROBE_WINDOW_CLOSED id={id:?} count={}",
                        self.windows_closed
                    );
                    if self.live_window_count() == 0 {
                        self.finish(
                            ProbeResult::Failed,
                            "all windows closed before probe completed".to_owned(),
                        )
                    } else {
                        Task::none()
                    }
                }
                window::Event::Resized(size) => {
                    self.resizes_seen += 1;
                    println!(
                        "BACKEND_SWITCH_PROBE_RESIZE_OBSERVED id={id:?} width={} height={} count={}",
                        size.width, size.height, self.resizes_seen
                    );
                    Task::none()
                }
                window::Event::RedrawRequested(at) => self.frame(Some(id), at),
                _ => Task::none(),
            }
        }

        fn frame(&mut self, id: Option<window::Id>, at: Instant) -> Task<Message> {
            self.frames_seen += 1;

            match self.state {
                ProbeState::WaitingInitialFrame => {
                    let required_windows = self.scenario.required_pre_switch_windows();

                    if self.window_open_requests < required_windows {
                        return self.open_extra_window();
                    }

                    if self.live_window_count() < required_windows as usize
                        && self.windows_opened < required_windows
                    {
                        return Task::none();
                    }

                    self.state = ProbeState::Switching;
                    self.switch_started = true;
                    self.switch_started_at = Some(at);
                    println!(
                        "BACKEND_SWITCH_PROBE_INITIAL_FRAME window={} frame={} elapsed_ms={}",
                        optional_window_id(id),
                        self.frames_seen,
                        elapsed_ms(self.started_at, at)
                    );

                    let switch = self.switch_backend(backend::Settings {
                        backend: Backend::Hardware(Api::Best),
                        antialiasing: false,
                        vsync: true,
                    });

                    Task::batch([switch, self.scenario_action_during_prepare()])
                }
                ProbeState::WaitingPostSwitchFrame => {
                    let Some(id) = self.post_switch_frame_window(id) else {
                        return Task::none();
                    };

                    if let Some(record) = self.windows.iter_mut().find(|record| record.id == id) {
                        record.post_switch_frame_seen = true;
                    }
                    println!(
                        "BACKEND_SWITCH_PROBE_POST_SWITCH_FRAME window={id:?} frame={} elapsed_after_config_ms={}",
                        self.frames_seen,
                        self.configured_at
                            .map(|configured_at| elapsed_ms(configured_at, at))
                            .unwrap_or(0)
                    );

                    if self.live_windows_need_post_switch_frames() {
                        Task::none()
                    } else {
                        let evidence = self.trace_evidence();
                        let (result, reason) = self.final_result_from_evidence(evidence.as_ref());
                        self.finish(result, reason)
                    }
                }
                ProbeState::Switching | ProbeState::Done => Task::none(),
            }
        }

        fn strict_handoff_completed(
            &mut self,
            outcome: backend::StrictHandoffOutcome,
        ) -> Task<Message> {
            self.configured_at = Some(Instant::now());
            let (result, reason) = match &outcome {
                Ok(strict_result) => {
                    println!(
                        "BACKEND_SWITCH_PROBE_STRICT_RESULT phase={} rollback={} windows={}",
                        strict_phase_value(strict_result.completed_phase),
                        strict_rollback_value(strict_result.rollback),
                        strict_result.windows.len()
                    );
                    self.final_result_from_strict_result(strict_result)
                }
                Err(error) => {
                    println!(
                        "BACKEND_SWITCH_PROBE_STRICT_ERROR phase={} category={} rollback={} message=\"{}\"",
                        strict_phase_value(error.phase),
                        strict_failure_category_value(error.category),
                        strict_rollback_value(error.rollback),
                        error.message
                    );
                    self.final_result_from_strict_error(error)
                }
            };

            self.strict_outcome = Some(outcome);
            self.finish(result, reason)
        }

        fn open_extra_window(&mut self) -> Task<Message> {
            self.window_open_requests += 1;
            let (_, open) = window::open(window::Settings {
                size: Size::new(300.0, 140.0),
                ..window::Settings::default()
            });

            open.map(Message::WindowOpened)
        }

        fn scenario_action_during_prepare(&mut self) -> Task<Message> {
            match self.scenario {
                ProbeScenario::ResizeDuringPreparing if !self.resize_requested => {
                    let Some(id) = self.first_live_window() else {
                        return Task::none();
                    };
                    self.resize_requested = true;
                    window::resize(id, Size::new(520.0, 260.0))
                }
                ProbeScenario::CloseDuringPreparing if !self.close_requested => {
                    let Some(id) = self.extra_live_window() else {
                        return Task::none();
                    };
                    self.close_requested = true;
                    window::close(id)
                }
                _ => Task::none(),
            }
        }

        fn finish(&mut self, result: ProbeResult, reason: String) -> Task<Message> {
            self.finished_from_state = Some(self.state);
            self.state = ProbeState::Done;
            self.result = result;
            self.reason = reason;

            if self.result == ProbeResult::Ok {
                println!(
                    "BACKEND_SWITCH_PROBE_DONE result=ok mode={} failure={} scenario={} reason=\"{}\"",
                    self.mode.marker_value(),
                    self.failure.marker_value(),
                    self.scenario.marker_value(),
                    self.reason
                );
            } else {
                println!(
                    "BACKEND_SWITCH_PROBE_DONE result={} mode={} failure={} scenario={} reason=\"{}\"",
                    self.result.marker_value(),
                    self.mode.marker_value(),
                    self.failure.marker_value(),
                    self.scenario.marker_value(),
                    self.reason
                );
            }

            self.write_result_log();
            iced::exit()
        }

        fn switch_backend(&self, settings: backend::Settings) -> Task<Message> {
            match self.mode {
                ProbeMode::PrepareWarmCommit => {
                    backend::prepare_warm_and_commit(settings).map(Message::StrictHandoffCompleted)
                }
                ProbeMode::Configure => {
                    backend::configure(settings).map(Message::BackendConfigured)
                }
            }
        }

        fn check_timeout(&mut self, now: Instant) -> Task<Message> {
            match self.state {
                ProbeState::WaitingInitialFrame
                    if now.duration_since(self.started_at) >= INITIAL_FRAME_TIMEOUT =>
                {
                    println!(
                        "BACKEND_SWITCH_PROBE_INITIAL_FRAME_TIMEOUT elapsed_ms={}",
                        elapsed_ms(self.started_at, now)
                    );
                    self.finish(
                        ProbeResult::Failed,
                        format!(
                            "initial frame timeout after {} ms",
                            elapsed_ms(self.started_at, now)
                        ),
                    )
                }
                ProbeState::Switching
                    if self.switch_started
                        && self.switch_started_at.is_some_and(|switch_started_at| {
                            now.duration_since(switch_started_at) >= STRICT_SWITCH_TIMEOUT
                        }) =>
                {
                    println!(
                        "BACKEND_SWITCH_PROBE_STRICT_SWITCH_TIMEOUT elapsed_ms={} state={:?}",
                        self.switch_started_at
                            .map(|switch_started_at| elapsed_ms(switch_started_at, now))
                            .unwrap_or(0),
                        self.state
                    );
                    self.finish(
                        ProbeResult::Indeterminate,
                        format!(
                            "strict handoff switching timeout after {} ms",
                            self.switch_started_at
                                .map(|switch_started_at| elapsed_ms(switch_started_at, now))
                                .unwrap_or(0)
                        ),
                    )
                }
                ProbeState::WaitingPostSwitchFrame
                    if self.configured_at.is_some_and(|configured_at| {
                        now.duration_since(configured_at) >= POST_SWITCH_FRAME_TIMEOUT
                    }) =>
                {
                    println!(
                        "BACKEND_SWITCH_PROBE_POST_SWITCH_FRAME_TIMEOUT elapsed_after_config_ms={}",
                        self.configured_at
                            .map(|configured_at| elapsed_ms(configured_at, now))
                            .unwrap_or(0)
                    );
                    self.finish(
                        ProbeResult::Failed,
                        format!(
                            "post-switch frame timeout after {} ms",
                            self.configured_at
                                .map(|configured_at| elapsed_ms(configured_at, now))
                                .unwrap_or(0)
                        ),
                    )
                }
                _ => Task::none(),
            }
        }

        fn subscription(&self) -> Subscription<Message> {
            Subscription::batch([
                window::events().map(|(id, event)| Message::WindowEvent(id, event)),
                window::frames().map(Message::Frame),
                iced::time::every(Duration::from_secs(1)).map(Message::Tick),
            ])
        }

        fn view(&self, _window: window::Id) -> Element<'_, Message> {
            center(
                column![
                    text("Backend switch probe").size(18),
                    text(format!("mode: {}", self.mode.label())).size(14),
                    text(format!("scenario: {}", self.scenario.marker_value())).size(14),
                    text(format!("failure: {}", self.failure.marker_value())).size(14),
                    text(format!("state: {:?}", self.state)).size(14),
                    text(format!("frames: {}", self.frames_seen)).size(14),
                    text(format!("windows open: {}", self.live_window_count())).size(14),
                ]
                .spacing(8),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }

        fn title(&self, _window: window::Id) -> String {
            format!("Backend switch probe ({})", self.scenario.marker_value())
        }

        fn live_window_count(&self) -> usize {
            self.windows.iter().filter(|record| record.live).count()
        }

        fn live_window_ids(&self) -> Vec<window::Id> {
            self.windows
                .iter()
                .filter_map(|record| record.live.then_some(record.id))
                .collect()
        }

        fn first_live_window(&self) -> Option<window::Id> {
            self.windows
                .iter()
                .find_map(|record| record.live.then_some(record.id))
        }

        fn extra_live_window(&self) -> Option<window::Id> {
            self.windows
                .iter()
                .filter(|record| record.live)
                .nth(1)
                .map(|record| record.id)
        }

        fn live_windows_need_post_switch_frames(&self) -> bool {
            self.windows
                .iter()
                .any(|record| record.live && !record.post_switch_frame_seen)
        }

        fn post_switch_frame_window(&self, id: Option<window::Id>) -> Option<window::Id> {
            if let Some(id) = id {
                return Some(id);
            }

            let live_windows = self.live_window_ids();

            match live_windows.as_slice() {
                [id] => Some(*id),
                [] => None,
                _ => {
                    println!(
                        "BACKEND_SWITCH_PROBE_POST_SWITCH_FRAME_UNSCOPED frame={} live_windows={} note=\"window::frames tick is not per-window evidence\"",
                        self.frames_seen,
                        live_windows.len()
                    );
                    None
                }
            }
        }

        fn final_result_from_evidence(
            &self,
            evidence: Option<&TraceEvidence>,
        ) -> (ProbeResult, String) {
            if self.failure != ProbeFailureMode::None {
                return (
                    ProbeResult::Failed,
                    "failure injection did not reach the app boundary".to_owned(),
                );
            }

            match self.scenario {
                ProbeScenario::ResizeDuringPreparing if self.resizes_seen == 0 => (
                    ProbeResult::Failed,
                    "resize-during-preparing scenario did not observe a resize event".to_owned(),
                ),
                ProbeScenario::CloseDuringPreparing if self.windows_closed == 0 => (
                    ProbeResult::Failed,
                    "close-during-preparing scenario did not observe a closed window".to_owned(),
                ),
                ProbeScenario::CloseDuringCommitPending => match evidence {
                    Some(evidence) if evidence.close_after_commit_pending_before_commit => (
                        ProbeResult::Ok,
                        "trace observed close after commit pending and before commit start"
                            .to_owned(),
                    ),
                    Some(_) => (
                        ProbeResult::Indeterminate,
                        "trace did not prove close occurred after commit pending and before commit start"
                            .to_owned(),
                    ),
                    None => (
                        ProbeResult::Indeterminate,
                        "commit-pending close timing requires FRAGILE_PERF_TRACE=1 evidence"
                            .to_owned(),
                    ),
                },
                _ => (
                    ProbeResult::Ok,
                    "basic configure observed post-switch frames for live windows; not strict proof"
                        .to_owned(),
                ),
            }
        }

        fn final_result_from_strict_result(
            &self,
            result: &backend::StrictHandoffResult,
        ) -> (ProbeResult, String) {
            if self.failure != ProbeFailureMode::None {
                return (
                    ProbeResult::Failed,
                    format!(
                        "expected injected {} failure but strict handoff completed",
                        self.failure.marker_value()
                    ),
                );
            }

            if result.completed_phase != backend::StrictHandoffPhase::Completed {
                return (
                    ProbeResult::Failed,
                    format!(
                        "strict handoff completed with phase {} instead of completed",
                        strict_phase_value(result.completed_phase)
                    ),
                );
            }

            if result.rollback != backend::StrictRollbackStatus::ReleasedAfterSuccess {
                return (
                    ProbeResult::Failed,
                    format!(
                        "strict handoff rollback status was {} instead of released_after_success",
                        strict_rollback_value(result.rollback)
                    ),
                );
            }

            let required_windows = self.live_window_ids();

            if required_windows.is_empty() {
                return (
                    ProbeResult::Failed,
                    "strict handoff completed with no live windows to validate".to_owned(),
                );
            }

            for id in &required_windows {
                if !result.windows.iter().any(|evidence| evidence.window == *id) {
                    return (
                        ProbeResult::Failed,
                        format!("strict handoff missing present evidence for live window {id:?}"),
                    );
                }

                if !result.windows.iter().any(|evidence| {
                    evidence.window == *id
                        && evidence.renderer_family == backend::RendererFamily::Wgpu
                        && evidence.status == backend::PresentStatus::Presented
                }) {
                    return (
                        ProbeResult::Failed,
                        format!(
                            "strict handoff evidence for live window {id:?} was not wgpu/presented"
                        ),
                    );
                }
            }

            if let Some(classification) = self.strict_trace_evidence_result() {
                return classification;
            }

            match self.strict_scenario_result() {
                Some(classification) => classification,
                None => (
                    ProbeResult::Ok,
                    format!(
                        "strict handoff completed with wgpu presented evidence for {} live window(s)",
                        required_windows.len()
                    ),
                ),
            }
        }

        fn strict_trace_evidence_result(&self) -> Option<(ProbeResult, String)> {
            let evidence = match self.trace_evidence() {
                Some(evidence) => evidence,
                None => {
                    return Some((
                        ProbeResult::Indeterminate,
                        "strict handoff success requires FRAGILE_PERF_TRACE=1 evidence".to_owned(),
                    ));
                }
            };

            if evidence.warm_complete_us.is_none() {
                return Some((
                    ProbeResult::Indeterminate,
                    "strict handoff trace did not include backend_handoff_warm_complete".to_owned(),
                ));
            }

            if evidence.warm_submission_completed != Some(true) {
                return Some((
                    ProbeResult::Failed,
                    format!(
                        "strict handoff warm-up submission_completed was {}",
                        optional_bool_marker(evidence.warm_submission_completed)
                    ),
                ));
            }

            if evidence.warm_renderer_family.as_deref() != Some("Wgpu") {
                return Some((
                    ProbeResult::Failed,
                    format!(
                        "strict handoff warm-up renderer family was {} instead of Wgpu",
                        evidence
                            .warm_renderer_family
                            .as_deref()
                            .unwrap_or("missing")
                    ),
                ));
            }

            if !matches!(
                evidence.warm_complete_us.zip(evidence.commit_pending_us),
                Some((warm_complete, commit_pending)) if warm_complete < commit_pending
            ) {
                return Some((
                    ProbeResult::Indeterminate,
                    "strict handoff trace did not prove warm-up completed before commit pending"
                        .to_owned(),
                ));
            }

            if evidence.software_frames_during_prepare == 0 {
                return Some((
                    ProbeResult::Indeterminate,
                    "strict handoff trace did not prove software frames during prepare".to_owned(),
                ));
            }

            if evidence.software_frames_after_commit_pending == 0 {
                return Some((
                    ProbeResult::Indeterminate,
                    "strict handoff trace did not prove software frames after commit pending"
                        .to_owned(),
                ));
            }

            if !evidence.first_post_commit_backend_is_hardware() {
                return Some((
                    ProbeResult::Indeterminate,
                    format!(
                        "strict handoff first post-commit backend was {} instead of hardware",
                        evidence
                            .first_post_commit_backend
                            .as_deref()
                            .unwrap_or("missing")
                    ),
                ));
            }

            None
        }

        fn final_result_from_strict_error(
            &self,
            error: &backend::StrictHandoffError,
        ) -> (ProbeResult, String) {
            if self.failure.matches_strict_error(error) {
                (
                    ProbeResult::Ok,
                    format!(
                        "injected {} failure observed as strict category {} with rollback {}",
                        self.failure.marker_value(),
                        strict_failure_category_value(error.category),
                        strict_rollback_value(error.rollback)
                    ),
                )
            } else if strict_error_is_intentional_close_during_preparing(
                self.scenario,
                self.failure,
                self.close_requested,
                self.windows_closed,
                error,
            ) {
                (
                    ProbeResult::Ok,
                    "close-during-preparing intentionally closed the extra window and strict handoff cancelled during prepare without rollback"
                        .to_owned(),
                )
            } else if self.failure != ProbeFailureMode::None {
                (
                    ProbeResult::Failed,
                    format!(
                        "expected injected {} failure but strict category was {}",
                        self.failure.marker_value(),
                        strict_failure_category_value(error.category)
                    ),
                )
            } else if strict_error_is_missing_first_present_evidence(error) {
                (
                    ProbeResult::Failed,
                    format!(
                        "strict handoff failed with structured non-timeout first-present evidence issue: {} (phase {}, category {}, rollback {}, windows={})",
                        error.message,
                        strict_phase_value(error.phase),
                        strict_failure_category_value(error.category),
                        strict_rollback_value(error.rollback),
                        error.windows.len()
                    ),
                )
            } else {
                (
                    ProbeResult::Failed,
                    format!(
                        "strict handoff failed in phase {} with category {} and rollback {}",
                        strict_phase_value(error.phase),
                        strict_failure_category_value(error.category),
                        strict_rollback_value(error.rollback)
                    ),
                )
            }
        }

        fn strict_scenario_result(&self) -> Option<(ProbeResult, String)> {
            match self.scenario {
                ProbeScenario::ResizeDuringPreparing if self.resizes_seen == 0 => Some((
                    ProbeResult::Failed,
                    "resize-during-preparing scenario did not observe a resize event".to_owned(),
                )),
                ProbeScenario::CloseDuringPreparing if self.windows_closed == 0 => Some((
                    ProbeResult::Failed,
                    "close-during-preparing scenario did not observe a closed window".to_owned(),
                )),
                ProbeScenario::CloseDuringCommitPending => match self.trace_evidence() {
                    Some(evidence) if evidence.close_after_commit_pending_before_commit => None,
                    Some(_) => Some((
                        ProbeResult::Indeterminate,
                        "trace did not prove close occurred after commit pending and before commit start"
                            .to_owned(),
                    )),
                    None => Some((
                        ProbeResult::Indeterminate,
                        "commit-pending close timing requires FRAGILE_PERF_TRACE=1 evidence"
                            .to_owned(),
                    )),
                },
                _ => None,
            }
        }

        fn trace_evidence(&self) -> Option<TraceEvidence> {
            let trace = self.trace_path.as_ref()?;
            let events = read_trace_events(trace).ok()?;
            Some(TraceEvidence::from_events(&events))
        }

        fn state_marker_value(&self) -> &'static str {
            match self.finished_from_state.unwrap_or(self.state) {
                ProbeState::WaitingInitialFrame => "waiting_initial_frame",
                ProbeState::Switching => "switching",
                ProbeState::WaitingPostSwitchFrame => "waiting_post_switch_frame",
                ProbeState::Done => "done",
            }
        }

        fn write_result_log(&mut self) {
            if self.result_written {
                return;
            }

            self.result_written = true;
            let trace_evidence = self.trace_evidence();
            let trace_path = self.trace_path.as_ref().map(|trace| trace.path.as_path());
            let content = self.result_json(trace_path, trace_evidence.as_ref());

            if let Some(parent) = self.result_path.parent()
                && let Err(error) = fs::create_dir_all(parent)
            {
                println!(
                    "BACKEND_SWITCH_PROBE_RESULT_LOG_FAILED path={} error={error}",
                    self.result_path.display()
                );
                return;
            }

            match fs::write(&self.result_path, content) {
                Ok(()) => println!(
                    "BACKEND_SWITCH_PROBE_RESULT_LOG path={}",
                    self.result_path.display()
                ),
                Err(error) => println!(
                    "BACKEND_SWITCH_PROBE_RESULT_LOG_FAILED path={} error={error}",
                    self.result_path.display()
                ),
            }
        }

        fn result_json(
            &self,
            trace_path: Option<&Path>,
            evidence: Option<&TraceEvidence>,
        ) -> String {
            let trace_path = trace_path
                .map(|path| json_string(&path.display().to_string()))
                .unwrap_or_else(|| String::from("null"));
            let evidence_json = evidence
                .map(TraceEvidence::to_json)
                .unwrap_or_else(|| String::from("{}"));
            let strict_outcome_json = strict_outcome_json(self.strict_outcome.as_ref());

            format!(
                concat!(
                    "{{\n",
                    "  \"mode\": {},\n",
                    "  \"scenario\": {},\n",
                    "  \"failure\": {},\n",
                    "  \"os\": {},\n",
                    "  \"arch\": {},\n",
                    "  \"result\": {},\n",
                    "  \"reason\": {},\n",
                    "  \"state\": {},\n",
                    "  \"final_state\": {},\n",
                    "  \"strict_outcome\": {},\n",
                    "  \"frames_seen\": {},\n",
                    "  \"windows_opened\": {},\n",
                    "  \"windows_closed\": {},\n",
                    "  \"resizes_seen\": {},\n",
                    "  \"trace_path\": {},\n",
                    "  \"trace_evidence\": {}\n",
                    "}}\n"
                ),
                json_string(self.mode.marker_value()),
                json_string(self.scenario.marker_value()),
                json_string(self.failure.marker_value()),
                json_string(std::env::consts::OS),
                json_string(std::env::consts::ARCH),
                json_string(self.result.marker_value()),
                json_string(&self.reason),
                json_string(self.state_marker_value()),
                json_string(match self.state {
                    ProbeState::WaitingInitialFrame => "waiting_initial_frame",
                    ProbeState::Switching => "switching",
                    ProbeState::WaitingPostSwitchFrame => "waiting_post_switch_frame",
                    ProbeState::Done => "done",
                }),
                strict_outcome_json,
                self.frames_seen,
                self.windows_opened,
                self.windows_closed,
                self.resizes_seen,
                trace_path,
                evidence_json
            )
        }
    }

    fn elapsed_ms(start: Instant, end: Instant) -> u128 {
        end.saturating_duration_since(start).as_millis()
    }

    impl ProbeMode {
        fn from_env_and_args() -> Self {
            std::env::args()
                .find_map(|arg| arg.strip_prefix("--mode=").map(str::to_owned))
                .or_else(|| std::env::var(MODE_ENV).ok())
                .as_deref()
                .map(Self::parse)
                .unwrap_or(Self::PrepareWarmCommit)
        }

        fn parse(value: &str) -> Self {
            match value {
                "configure" | "basic-configure" => Self::Configure,
                _ => Self::PrepareWarmCommit,
            }
        }

        fn marker_value(self) -> &'static str {
            match self {
                Self::PrepareWarmCommit => "prepare_warm_commit",
                Self::Configure => "configure",
            }
        }

        fn label(self) -> &'static str {
            match self {
                Self::PrepareWarmCommit => "prepare/warm/commit",
                Self::Configure => "configure",
            }
        }
    }

    impl ProbeFailureMode {
        fn from_env_and_args() -> Self {
            std::env::args()
                .find_map(|arg| arg.strip_prefix("--fail=").map(str::to_owned))
                .or_else(|| std::env::var(FAILURE_ENV).ok())
                .as_deref()
                .map(Self::parse)
                .unwrap_or(Self::None)
        }

        fn parse(value: &str) -> Self {
            match value {
                "prepare" => Self::Prepare,
                "warm" | "warm-up" => Self::Warm,
                "commit" => Self::Commit,
                "first-present" | "first_present" => Self::FirstPresent,
                _ => Self::None,
            }
        }

        fn install_runtime_env(self) {
            let Some(value) = self.runtime_value() else {
                return;
            };

            // The probe is still single-threaded and has not started Iced yet.
            unsafe {
                std::env::set_var(RUNTIME_FAILURE_ENV, value);
            }
        }

        fn runtime_value(self) -> Option<&'static str> {
            match self {
                Self::None => None,
                Self::Prepare => Some("prepare"),
                Self::Warm => Some("warm"),
                Self::Commit => Some("commit"),
                Self::FirstPresent => Some("first-present"),
            }
        }

        fn marker_value(self) -> &'static str {
            self.runtime_value().unwrap_or("none")
        }

        fn matches_error(self, error: &backend::Error) -> bool {
            let Some(value) = self.runtime_value() else {
                return false;
            };

            format!("{error:?}").contains(value)
        }

        fn matches_strict_error(self, error: &backend::StrictHandoffError) -> bool {
            match self {
                Self::None => false,
                Self::Prepare => error.category == backend::StrictHandoffFailureCategory::Prepare,
                Self::Warm => error.category == backend::StrictHandoffFailureCategory::WarmUp,
                Self::Commit => error.category == backend::StrictHandoffFailureCategory::Commit,
                Self::FirstPresent => matches!(
                    error.category,
                    backend::StrictHandoffFailureCategory::FirstPresent
                        | backend::StrictHandoffFailureCategory::RendererEvidenceMissing
                ),
            }
        }
    }

    impl ProbeScenario {
        fn from_env_and_args() -> Self {
            std::env::args()
                .find_map(|arg| arg.strip_prefix("--scenario=").map(str::to_owned))
                .or_else(|| std::env::var(SCENARIO_ENV).ok())
                .as_deref()
                .map(Self::parse)
                .unwrap_or(Self::SingleWindow)
        }

        fn parse(value: &str) -> Self {
            match value {
                "multi-window" => Self::MultiWindow,
                "resize-during-preparing" => Self::ResizeDuringPreparing,
                "close-during-preparing" => Self::CloseDuringPreparing,
                "close-during-commit-pending" => Self::CloseDuringCommitPending,
                _ => Self::SingleWindow,
            }
        }

        fn install_runtime_env(self) {
            if self.needs_prepare_delay() && std::env::var_os(RUNTIME_PREPARE_DELAY_ENV).is_none() {
                // The probe is still single-threaded and has not started Iced yet.
                unsafe {
                    std::env::set_var(RUNTIME_PREPARE_DELAY_ENV, PREPARE_DELAY_MS);
                }
            }

            if self.needs_commit_pending_delay()
                && std::env::var_os(RUNTIME_COMMIT_PENDING_DELAY_ENV).is_none()
            {
                // Probe-only diagnostic hook: keep commit pending open long enough to
                // request a normal app close before the runtime commits the handoff.
                unsafe {
                    std::env::set_var(RUNTIME_COMMIT_PENDING_DELAY_ENV, COMMIT_PENDING_DELAY_MS);
                }
            }

            if self.needs_commit_pending_delay() && std::env::var_os(RUNTIME_TRACE_ENV).is_none() {
                // The commit-pending scenario uses trace as a phase signal and
                // as final evidence for close-before-commit timing.
                unsafe {
                    std::env::set_var(RUNTIME_TRACE_ENV, "1");
                }
            }

            if self.needs_commit_pending_delay()
                && std::env::var_os(RUNTIME_COMMIT_PENDING_CLOSE_ENV).is_none()
            {
                // Probe-only diagnostic hook: close an extra live window after
                // commit pending is established and before handoff commit.
                unsafe {
                    std::env::set_var(RUNTIME_COMMIT_PENDING_CLOSE_ENV, "1");
                }
            }
        }

        fn needs_prepare_delay(self) -> bool {
            matches!(
                self,
                Self::ResizeDuringPreparing | Self::CloseDuringPreparing
            )
        }

        fn needs_commit_pending_delay(self) -> bool {
            matches!(self, Self::CloseDuringCommitPending)
        }

        fn required_pre_switch_windows(self) -> u64 {
            match self {
                Self::SingleWindow | Self::ResizeDuringPreparing => 1,
                Self::MultiWindow | Self::CloseDuringPreparing | Self::CloseDuringCommitPending => {
                    2
                }
            }
        }

        fn marker_value(self) -> &'static str {
            match self {
                Self::SingleWindow => "single-window",
                Self::MultiWindow => "multi-window",
                Self::ResizeDuringPreparing => "resize-during-preparing",
                Self::CloseDuringPreparing => "close-during-preparing",
                Self::CloseDuringCommitPending => "close-during-commit-pending",
            }
        }

        fn stable_suffix(self) -> &'static str {
            match self {
                Self::SingleWindow => "SINGLE_WINDOW",
                Self::MultiWindow => "MULTI_WINDOW",
                Self::ResizeDuringPreparing => "RESIZE_DURING_PREPARING",
                Self::CloseDuringPreparing => "CLOSE_DURING_PREPARING",
                Self::CloseDuringCommitPending => "CLOSE_DURING_COMMIT_PENDING",
            }
        }
    }

    impl ProbeResult {
        fn marker_value(self) -> &'static str {
            match self {
                Self::Ok => "ok",
                Self::Failed => "failed",
                Self::Indeterminate => "indeterminate",
            }
        }
    }

    fn result_path(mode: ProbeMode, scenario: ProbeScenario, failure: ProbeFailureMode) -> PathBuf {
        let dir = std::env::var_os(RESULT_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target").join("hybrid-rendering-probes"));

        dir.join(format!(
            "{}-{}-{}.json",
            mode.marker_value(),
            scenario.marker_value(),
            failure.marker_value()
        ))
    }

    struct TraceValidation {
        path: PathBuf,
        started_at_us: u128,
    }

    impl Clone for TraceValidation {
        fn clone(&self) -> Self {
            Self {
                path: self.path.clone(),
                started_at_us: self.started_at_us,
            }
        }
    }

    #[derive(Debug)]
    struct TraceEvidence {
        prepare_start_us: Option<u128>,
        warm_start_us: Option<u128>,
        commit_pending_us: Option<u128>,
        commit_start_us: Option<u128>,
        commit_complete_us: Option<u128>,
        warm_complete_us: Option<u128>,
        warm_elapsed_us: Option<u128>,
        warm_renderer_family: Option<String>,
        warm_backend: Option<String>,
        warm_adapter: Option<String>,
        warm_passes: Option<u128>,
        warm_submission_completed: Option<bool>,
        warm_timeout_ms: Option<u128>,
        warm_failure: Option<String>,
        prepare_to_warm_ms: Option<u128>,
        commit_pending_to_start_ms: Option<u128>,
        software_frames_during_prepare: usize,
        software_frames_after_commit_pending: usize,
        first_post_commit_backend: Option<String>,
        close_after_commit_pending_before_commit: bool,
    }

    impl TraceEvidence {
        fn from_events(events: &[TraceEvent]) -> Self {
            let prepare_start_us = event_time(events, "backend_handoff_prepare_start");
            let warm_start_us = event_time(events, "backend_handoff_warm_start");
            let commit_pending_us = event_time(events, "backend_handoff_commit_pending");
            let commit_start_us = event_time(events, "backend_handoff_commit_start");
            let commit_complete_us = event_time(events, "backend_handoff_commit_complete");
            let warm_complete = events
                .iter()
                .find(|event| event.event == "backend_handoff_warm_complete");
            let warm_failed = events
                .iter()
                .find(|event| event.event == "backend_handoff_warm_failed");
            let warm_event = warm_complete.or(warm_failed);

            let software_frames_during_prepare = if let (Some(prepare_start), Some(warm_start)) =
                (prepare_start_us, warm_start_us)
            {
                events
                    .iter()
                    .filter(|event| {
                        event.timestamp_us > prepare_start
                            && event.timestamp_us < warm_start
                            && event.event == "fallback_present"
                            && backend_is(&event.detail, "tiny-skia")
                            && status_is_ok(&event.detail)
                    })
                    .count()
            } else {
                0
            };

            let software_frames_after_commit_pending =
                if let (Some(commit_pending), Some(commit_start)) =
                    (commit_pending_us, commit_start_us)
                {
                    events
                        .iter()
                        .filter(|event| {
                            event.timestamp_us >= commit_pending
                                && event.timestamp_us <= commit_start
                                && event.event == "fallback_present"
                                && backend_is(&event.detail, "tiny-skia")
                                && status_is_ok(&event.detail)
                        })
                        .count()
                } else {
                    0
                };

            let first_post_commit_backend = commit_complete_us.and_then(|commit_complete| {
                events
                    .iter()
                    .find(|event| {
                        event.timestamp_us > commit_complete
                            && event.event == "fallback_present"
                            && status_is_ok(&event.detail)
                    })
                    .and_then(|event| backend_value(&event.detail).map(str::to_owned))
            });

            let close_after_commit_pending_before_commit =
                if let (Some(commit_pending), Some(commit_start)) =
                    (commit_pending_us, commit_start_us)
                {
                    events.iter().any(|event| {
                        event.timestamp_us >= commit_pending
                            && event.timestamp_us <= commit_start
                            && event.event == "winit_window_close"
                    })
                } else {
                    false
                };

            Self {
                prepare_start_us,
                warm_start_us,
                commit_pending_us,
                commit_start_us,
                commit_complete_us,
                warm_complete_us: warm_complete.map(|event| event.timestamp_us),
                warm_elapsed_us: warm_event
                    .and_then(|event| detail_u128(&event.detail, "elapsed_us")),
                warm_renderer_family: warm_event
                    .and_then(|event| detail_value(&event.detail, "renderer_family="))
                    .map(str::to_owned),
                warm_backend: warm_event
                    .and_then(|event| detail_value(&event.detail, "backend="))
                    .map(str::to_owned),
                warm_adapter: warm_event
                    .and_then(|event| detail_between(&event.detail, "adapter=", " backend="))
                    .map(str::to_owned),
                warm_passes: warm_event.and_then(|event| detail_u128(&event.detail, "passes")),
                warm_submission_completed: warm_event
                    .and_then(|event| detail_bool(&event.detail, "submission_completed")),
                warm_timeout_ms: warm_event
                    .and_then(|event| detail_u128(&event.detail, "timeout_ms")),
                warm_failure: warm_failed
                    .and_then(|event| detail_after(&event.detail, "error="))
                    .map(str::to_owned),
                prepare_to_warm_ms: prepare_start_us
                    .zip(warm_start_us)
                    .map(|(start, end)| end.saturating_sub(start) / 1_000),
                commit_pending_to_start_ms: commit_pending_us
                    .zip(commit_start_us)
                    .map(|(pending, start)| start.saturating_sub(pending) / 1_000),
                software_frames_during_prepare,
                software_frames_after_commit_pending,
                first_post_commit_backend,
                close_after_commit_pending_before_commit,
            }
        }

        fn first_post_commit_backend_is_hardware(&self) -> bool {
            self.first_post_commit_backend
                .as_deref()
                .is_some_and(|backend| backend != "tiny-skia")
        }

        fn to_json(&self) -> String {
            format!(
                concat!(
                    "{{",
                    "\"prepare_start_us\":{},",
                    "\"warm_start_us\":{},",
                    "\"commit_pending_us\":{},",
                    "\"commit_start_us\":{},",
                    "\"commit_complete_us\":{},",
                    "\"warm_complete_us\":{},",
                    "\"warm_elapsed_us\":{},",
                    "\"warm_renderer_family\":{},",
                    "\"warm_backend\":{},",
                    "\"warm_adapter\":{},",
                    "\"warm_passes\":{},",
                    "\"warm_submission_completed\":{},",
                    "\"warm_timeout_ms\":{},",
                    "\"warm_failure\":{},",
                    "\"prepare_to_warm_ms\":{},",
                    "\"commit_pending_to_start_ms\":{},",
                    "\"software_frames_during_prepare\":{},",
                    "\"software_frames_after_commit_pending\":{},",
                    "\"first_post_commit_backend\":{},",
                    "\"close_after_commit_pending_before_commit\":{}",
                    "}}"
                ),
                json_optional_u128(self.prepare_start_us),
                json_optional_u128(self.warm_start_us),
                json_optional_u128(self.commit_pending_us),
                json_optional_u128(self.commit_start_us),
                json_optional_u128(self.commit_complete_us),
                json_optional_u128(self.warm_complete_us),
                json_optional_u128(self.warm_elapsed_us),
                self.warm_renderer_family
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| String::from("null")),
                self.warm_backend
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| String::from("null")),
                self.warm_adapter
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| String::from("null")),
                json_optional_u128(self.warm_passes),
                json_optional_bool(self.warm_submission_completed),
                json_optional_u128(self.warm_timeout_ms),
                self.warm_failure
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| String::from("null")),
                json_optional_u128(self.prepare_to_warm_ms),
                json_optional_u128(self.commit_pending_to_start_ms),
                self.software_frames_during_prepare,
                self.software_frames_after_commit_pending,
                self.first_post_commit_backend
                    .as_deref()
                    .map(json_string)
                    .unwrap_or_else(|| String::from("null")),
                self.close_after_commit_pending_before_commit
            )
        }
    }

    impl TraceValidation {
        fn install_for_probe(
            mode: ProbeMode,
            scenario: ProbeScenario,
            failure: ProbeFailureMode,
        ) -> Option<Self> {
            std::env::var_os(RUNTIME_TRACE_ENV)?;

            let dir = std::env::var_os(RUNTIME_TRACE_DIR_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    let dir = cargo_target_dir()
                        .join("perf")
                        .join(scenario.marker_value())
                        .join(format!(
                            "{}-{}",
                            mode.marker_value(),
                            failure.marker_value()
                        ));

                    // The probe is still single-threaded and has not started Iced yet.
                    unsafe {
                        std::env::set_var(RUNTIME_TRACE_DIR_ENV, &dir);
                    }

                    dir
                });

            Some(Self {
                path: dir.join("fragile-perf.csv"),
                started_at_us: timestamp_us(),
            })
        }

        fn remove_existing(&self) {
            match std::fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => println!(
                    "BACKEND_SWITCH_PROBE_TRACE_RESET_FAILED path={} error={error}",
                    self.path.display()
                ),
            }
        }
    }

    fn cargo_target_dir() -> PathBuf {
        std::env::var_os(CARGO_TARGET_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target"))
    }

    fn json_optional_u128(value: Option<u128>) -> String {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| String::from("null"))
    }

    fn json_optional_bool(value: Option<bool>) -> String {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| String::from("null"))
    }

    fn optional_bool_marker(value: Option<bool>) -> &'static str {
        match value {
            Some(true) => "true",
            Some(false) => "false",
            None => "missing",
        }
    }

    fn strict_error_is_intentional_close_during_preparing(
        scenario: ProbeScenario,
        failure: ProbeFailureMode,
        close_requested: bool,
        _windows_closed: u64,
        error: &backend::StrictHandoffError,
    ) -> bool {
        scenario == ProbeScenario::CloseDuringPreparing
            && failure == ProbeFailureMode::None
            && close_requested
            && error.category == backend::StrictHandoffFailureCategory::Cancelled
            && error.phase == backend::StrictHandoffPhase::Preparing
            && error.rollback == backend::StrictRollbackStatus::NotNeeded
    }

    fn strict_error_is_missing_first_present_evidence(error: &backend::StrictHandoffError) -> bool {
        error.phase == backend::StrictHandoffPhase::AwaitingFirstPresent
            && matches!(
                error.category,
                backend::StrictHandoffFailureCategory::FirstPresent
                    | backend::StrictHandoffFailureCategory::RendererEvidenceMissing
            )
    }

    fn strict_outcome_json(outcome: Option<&backend::StrictHandoffOutcome>) -> String {
        match outcome {
            Some(Ok(result)) => strict_result_json(result),
            Some(Err(error)) => strict_error_json(error),
            None => String::from("null"),
        }
    }

    fn strict_result_json(result: &backend::StrictHandoffResult) -> String {
        format!(
            concat!(
                "{{",
                "\"kind\":\"success\",",
                "\"completed_phase\":{},",
                "\"rollback\":{},",
                "\"windows\":{}",
                "}}"
            ),
            json_string(strict_phase_value(result.completed_phase)),
            json_string(strict_rollback_value(result.rollback)),
            strict_windows_json(&result.windows)
        )
    }

    fn strict_error_json(error: &backend::StrictHandoffError) -> String {
        format!(
            concat!(
                "{{",
                "\"kind\":\"error\",",
                "\"phase\":{},",
                "\"category\":{},",
                "\"rollback\":{},",
                "\"message\":{},",
                "\"windows\":{}",
                "}}"
            ),
            json_string(strict_phase_value(error.phase)),
            json_string(strict_failure_category_value(error.category)),
            json_string(strict_rollback_value(error.rollback)),
            json_string(&error.message),
            strict_windows_json(&error.windows)
        )
    }

    fn strict_windows_json(windows: &[backend::StrictHandoffWindowEvidence]) -> String {
        format!(
            "[{}]",
            windows
                .iter()
                .map(strict_window_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn strict_window_json(evidence: &backend::StrictHandoffWindowEvidence) -> String {
        let (status, surface_failure) = strict_present_status_values(&evidence.status);
        let surface_failure = surface_failure
            .map(json_string)
            .unwrap_or_else(|| String::from("null"));

        format!(
            concat!(
                "{{",
                "\"window\":{},",
                "\"frame_sequence\":{},",
                "\"renderer_family\":{},",
                "\"adapter\":{},",
                "\"backend\":{},",
                "\"status\":{},",
                "\"surface_failure\":{},",
                "\"phase\":{}",
                "}}"
            ),
            json_string(&format!("{:?}", evidence.window)),
            evidence.frame_sequence,
            json_string(strict_renderer_family_value(evidence.renderer_family)),
            evidence
                .adapter
                .as_deref()
                .map(json_string)
                .unwrap_or_else(|| String::from("null")),
            evidence
                .backend
                .as_deref()
                .map(json_string)
                .unwrap_or_else(|| String::from("null")),
            json_string(status),
            surface_failure,
            json_string(strict_phase_value(evidence.phase))
        )
    }

    fn strict_phase_value(phase: backend::StrictHandoffPhase) -> &'static str {
        match phase {
            backend::StrictHandoffPhase::Preparing => "preparing",
            backend::StrictHandoffPhase::Warming => "warming",
            backend::StrictHandoffPhase::CommitPending => "commit_pending",
            backend::StrictHandoffPhase::AwaitingFirstPresent => "awaiting_first_present",
            backend::StrictHandoffPhase::Completed => "completed",
            backend::StrictHandoffPhase::Cancelled => "cancelled",
            backend::StrictHandoffPhase::Rollback => "rollback",
        }
    }

    fn strict_failure_category_value(
        category: backend::StrictHandoffFailureCategory,
    ) -> &'static str {
        match category {
            backend::StrictHandoffFailureCategory::AlreadyInProgress => "already_in_progress",
            backend::StrictHandoffFailureCategory::NoActiveWindow => "no_active_window",
            backend::StrictHandoffFailureCategory::Prepare => "prepare",
            backend::StrictHandoffFailureCategory::WarmUp => "warm_up",
            backend::StrictHandoffFailureCategory::Commit => "commit",
            backend::StrictHandoffFailureCategory::FirstPresent => "first_present",
            backend::StrictHandoffFailureCategory::Cancelled => "cancelled",
            backend::StrictHandoffFailureCategory::Unsupported => "unsupported",
            backend::StrictHandoffFailureCategory::RendererEvidenceMissing => {
                "renderer_evidence_missing"
            }
            backend::StrictHandoffFailureCategory::RollbackFailed => "rollback_failed",
        }
    }

    fn strict_rollback_value(rollback: backend::StrictRollbackStatus) -> &'static str {
        match rollback {
            backend::StrictRollbackStatus::NotNeeded => "not_needed",
            backend::StrictRollbackStatus::Retained => "retained",
            backend::StrictRollbackStatus::Restored => "restored",
            backend::StrictRollbackStatus::RestoreFailed => "restore_failed",
            backend::StrictRollbackStatus::ReleasedAfterSuccess => "released_after_success",
            backend::StrictRollbackStatus::NotAvailable => "not_available",
        }
    }

    fn strict_renderer_family_value(renderer_family: backend::RendererFamily) -> &'static str {
        match renderer_family {
            backend::RendererFamily::TinySkia => "tiny_skia",
            backend::RendererFamily::Wgpu => "wgpu",
            backend::RendererFamily::Null => "null",
            backend::RendererFamily::Unknown => "unknown",
        }
    }

    fn strict_present_status_values(
        status: &backend::PresentStatus,
    ) -> (&'static str, Option<&'static str>) {
        match status {
            backend::PresentStatus::Presented => ("presented", None),
            backend::PresentStatus::Failed(failure) => (
                "failed",
                Some(match failure {
                    backend::SurfaceFailure::Timeout => "timeout",
                    backend::SurfaceFailure::Outdated => "outdated",
                    backend::SurfaceFailure::Lost => "lost",
                    backend::SurfaceFailure::OutOfMemory => "out_of_memory",
                    backend::SurfaceFailure::Occluded => "occluded",
                    backend::SurfaceFailure::Other => "other",
                }),
            ),
            backend::PresentStatus::ClosedBeforeProof => ("closed_before_proof", None),
        }
    }

    fn json_string(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len() + 2);
        escaped.push('"');

        for ch in value.chars() {
            match ch {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
                ch => escaped.push(ch),
            }
        }

        escaped.push('"');
        escaped
    }

    #[derive(Debug)]
    struct TraceEvent {
        timestamp_us: u128,
        event: String,
        detail: String,
    }

    fn read_trace_events(trace: &TraceValidation) -> Result<Vec<TraceEvent>, std::io::Error> {
        std::fs::read_to_string(&trace.path).map(|content| {
            content
                .lines()
                .filter_map(parse_trace_event)
                .filter(|event| event.timestamp_us >= trace.started_at_us)
                .collect::<Vec<_>>()
        })
    }

    fn parse_trace_event(line: &str) -> Option<TraceEvent> {
        if line.starts_with("timestamp_us,") {
            return None;
        }

        let mut fields = line.splitn(4, ',');
        let timestamp_us = fields.next()?.parse().ok()?;
        let event = fields.next()?.to_owned();
        let _elapsed_us = fields.next()?;
        let detail = fields.next()?.trim_matches('"').replace("\"\"", "\"");

        Some(TraceEvent {
            timestamp_us,
            event,
            detail,
        })
    }

    fn event_time(events: &[TraceEvent], name: &str) -> Option<u128> {
        events
            .iter()
            .find(|event| event.event == name)
            .map(|event| event.timestamp_us)
    }

    fn backend_is(detail: &str, expected: &str) -> bool {
        backend_value(detail).is_some_and(|backend| backend == expected)
    }

    fn backend_value(detail: &str) -> Option<&str> {
        detail_value(detail, "backend=")
    }

    fn detail_value<'a>(detail: &'a str, prefix: &str) -> Option<&'a str> {
        detail
            .split_whitespace()
            .find_map(|field| field.strip_prefix(prefix))
    }

    fn detail_between<'a>(detail: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
        let value = detail.split_once(prefix)?.1;
        value.split_once(suffix).map(|(value, _)| value)
    }

    fn detail_after<'a>(detail: &'a str, prefix: &str) -> Option<&'a str> {
        detail.split_once(prefix).map(|(_, value)| value)
    }

    fn detail_u128(detail: &str, key: &str) -> Option<u128> {
        detail_value(detail, &format!("{key}="))?.parse().ok()
    }

    fn detail_bool(detail: &str, key: &str) -> Option<bool> {
        match detail_value(detail, &format!("{key}="))? {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    fn status_is_ok(detail: &str) -> bool {
        detail.split_whitespace().any(|field| field == "status=ok")
    }

    fn optional_window_id(id: Option<window::Id>) -> String {
        id.map(|id| format!("{id:?}"))
            .unwrap_or_else(|| String::from("unscoped"))
    }

    fn timestamp_us() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_micros())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn strict_error(
            phase: backend::StrictHandoffPhase,
            category: backend::StrictHandoffFailureCategory,
            rollback: backend::StrictRollbackStatus,
        ) -> backend::StrictHandoffError {
            backend::StrictHandoffError {
                phase,
                category,
                rollback,
                windows: Vec::new(),
                message: "test strict handoff error".to_owned(),
            }
        }

        #[test]
        fn classifies_intentional_close_during_preparing_cancellation() {
            let error = strict_error(
                backend::StrictHandoffPhase::Preparing,
                backend::StrictHandoffFailureCategory::Cancelled,
                backend::StrictRollbackStatus::NotNeeded,
            );

            assert!(strict_error_is_intentional_close_during_preparing(
                ProbeScenario::CloseDuringPreparing,
                ProbeFailureMode::None,
                true,
                0,
                &error,
            ));
        }

        #[test]
        fn rejects_unexpected_close_during_preparing_cancellations() {
            let error = strict_error(
                backend::StrictHandoffPhase::Preparing,
                backend::StrictHandoffFailureCategory::Cancelled,
                backend::StrictRollbackStatus::NotNeeded,
            );

            assert!(!strict_error_is_intentional_close_during_preparing(
                ProbeScenario::MultiWindow,
                ProbeFailureMode::None,
                true,
                1,
                &error,
            ));
            assert!(!strict_error_is_intentional_close_during_preparing(
                ProbeScenario::CloseDuringPreparing,
                ProbeFailureMode::Prepare,
                true,
                1,
                &error,
            ));
            assert!(!strict_error_is_intentional_close_during_preparing(
                ProbeScenario::CloseDuringPreparing,
                ProbeFailureMode::None,
                false,
                0,
                &error,
            ));
        }

        #[test]
        fn rejects_cancellations_with_wrong_phase_or_rollback() {
            let wrong_phase = strict_error(
                backend::StrictHandoffPhase::Warming,
                backend::StrictHandoffFailureCategory::Cancelled,
                backend::StrictRollbackStatus::NotNeeded,
            );
            let wrong_rollback = strict_error(
                backend::StrictHandoffPhase::Preparing,
                backend::StrictHandoffFailureCategory::Cancelled,
                backend::StrictRollbackStatus::Restored,
            );

            assert!(!strict_error_is_intentional_close_during_preparing(
                ProbeScenario::CloseDuringPreparing,
                ProbeFailureMode::None,
                true,
                1,
                &wrong_phase,
            ));
            assert!(!strict_error_is_intentional_close_during_preparing(
                ProbeScenario::CloseDuringPreparing,
                ProbeFailureMode::None,
                true,
                1,
                &wrong_rollback,
            ));
        }

        #[test]
        fn recognizes_structured_first_present_evidence_errors() {
            let renderer_evidence_missing = strict_error(
                backend::StrictHandoffPhase::AwaitingFirstPresent,
                backend::StrictHandoffFailureCategory::RendererEvidenceMissing,
                backend::StrictRollbackStatus::Restored,
            );
            let first_present = strict_error(
                backend::StrictHandoffPhase::AwaitingFirstPresent,
                backend::StrictHandoffFailureCategory::FirstPresent,
                backend::StrictRollbackStatus::Restored,
            );
            let unrelated = strict_error(
                backend::StrictHandoffPhase::Preparing,
                backend::StrictHandoffFailureCategory::RendererEvidenceMissing,
                backend::StrictRollbackStatus::Restored,
            );

            assert!(strict_error_is_missing_first_present_evidence(
                &renderer_evidence_missing,
            ));
            assert!(strict_error_is_missing_first_present_evidence(
                &first_present
            ));
            assert!(!strict_error_is_missing_first_present_evidence(&unrelated));
        }
    }
}
