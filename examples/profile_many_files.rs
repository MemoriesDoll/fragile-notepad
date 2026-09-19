#[cfg(not(feature = "hybrid-rendering"))]
mod profile {
    use fragile_notepad::{
        app::App,
        core::{DecodedText, DocumentId, EditorSettings, TextEncoding},
        message::{Message, OpenedFile},
        services,
    };
    use iced::{Element, Subscription, Task, window};
    use std::{
        cell::Cell,
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };

    fn fixture(kind: &str) -> String {
        match kind {
            "rust64" => "fn sample() {\n    let value = 123;\n}\n".repeat(1800),
            "rust900" => "fn sample() {\n    let value = 123;\n}\n".repeat(26000),
            "rust1100" => "fn sample() {\n    let value = 123;\n}\n".repeat(33000),
            "text64" => "a plain text line with some words\n".repeat(2000),
            _ => "a plain text line with some words\n".repeat(120),
        }
    }

    fn isolated_directory() -> PathBuf {
        let variable = if cfg!(windows) {
            "APPDATA"
        } else {
            "XDG_CONFIG_HOME"
        };
        let dir = PathBuf::from(
            std::env::var_os(variable).expect("set an isolated profiler configuration directory"),
        );
        assert!(
            dir.is_absolute()
                && dir
                    .components()
                    .any(|part| part.as_os_str() == "many-files-investigation"),
            "profiler configuration must be inside a many-files-investigation directory"
        );
        dir
    }

    pub fn run() -> iced::Result {
        let args: Vec<_> = std::env::args().collect();
        if args.get(1).map(String::as_str) == Some("recovery") {
            return recovery(args.get(2).map(String::as_str) == Some("verify"));
        }
        if args.get(1).map(String::as_str) == Some("model") {
            model();
            return Ok(());
        }
        if args.get(1).map(String::as_str) == Some("history") {
            history();
            return Ok(());
        }
        if args.get(1).map(String::as_str) == Some("outline") {
            outline();
            return Ok(());
        }
        let count = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(100usize);
        let kind = args.get(2).cloned().unwrap_or_else(|| "text4".into());
        let text = fixture(&kind);
        let dir = isolated_directory();
        std::fs::create_dir_all(dir.join("fixtures")).unwrap();
        let paths: Vec<_> = (0..count)
            .map(|i| {
                let path = dir.join("fixtures").join(format!(
                    "document_{i:03}.{}",
                    if kind.starts_with("rust") {
                        "rs"
                    } else {
                        "txt"
                    }
                ));
                std::fs::write(&path, &text).unwrap();
                path
            })
            .collect();
        println!("LIVE count={count} kind={kind} bytes_each={}", text.len());
        iced::daemon(move || Live::new(paths.clone()), Live::update, Live::view)
            .subscription(Live::subscription)
            .theme(iced::Theme::Light)
            .settings(fragile_notepad::startup::iced_settings())
            .run()
    }

    #[derive(Debug, Clone)]
    enum Probe {
        App(Message),
        Start,
        Tick,
    }
    struct Live {
        app: App,
        paths: Vec<PathBuf>,
        started: Instant,
        loading: Option<Instant>,
        done: Option<Instant>,
        first_view: Cell<bool>,
        completed: usize,
        failed: usize,
        persisted: usize,
        persist_failed: usize,
        outline: usize,
        chunks: usize,
        handler_us: [u128; 4],
        max_handler_us: [u128; 4],
        max_tick_us: u128,
        last_tick: Instant,
    }
    impl Live {
        fn new(paths: Vec<PathBuf>) -> (Self, Task<Probe>) {
            let started = Instant::now();
            let (app, init) = App::new();
            let state = Self {
                app,
                paths,
                started,
                loading: None,
                done: None,
                first_view: Cell::new(false),
                completed: 0,
                failed: 0,
                persisted: 0,
                persist_failed: 0,
                outline: 0,
                chunks: 0,
                handler_us: [0; 4],
                max_handler_us: [0; 4],
                max_tick_us: 0,
                last_tick: started,
            };
            (
                state,
                Task::batch([
                    init.map(Probe::App),
                    Task::perform(
                        async {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        },
                        |_| Probe::Start,
                    ),
                ]),
            )
        }
        fn subscription(&self) -> Subscription<Probe> {
            Subscription::batch([
                self.app.subscription().map(Probe::App),
                iced::time::every(Duration::from_millis(16)).map(|_| Probe::Tick),
            ])
        }
        fn view(&self, id: window::Id) -> Element<'_, Probe> {
            if !self.first_view.replace(true) {
                println!(
                    "first_view_ms={:.2}",
                    self.started.elapsed().as_secs_f64() * 1000.0
                );
            }
            self.app.view(id).map(Probe::App)
        }
        fn update(&mut self, message: Probe) -> Task<Probe> {
            match message {
                Probe::Start => {
                    self.loading = Some(Instant::now());
                    self.last_tick = Instant::now();
                    self.max_tick_us = 0;
                    let start = Instant::now();
                    let tasks = self
                        .paths
                        .iter()
                        .map(|p| {
                            self.app
                                .update(Message::FilePicked(Ok(p.clone())))
                                .map(Probe::App)
                        })
                        .collect::<Vec<_>>();
                    println!(
                        "insert_loading_tabs_ms={:.2}",
                        start.elapsed().as_secs_f64() * 1000.0
                    );
                    Task::batch(tasks)
                }
                Probe::App(message) => {
                    let category = match &message {
                        Message::FileLoadChunk(_) => {
                            self.chunks += 1;
                            0
                        }
                        Message::FileLoadFinished(result) => {
                            self.completed += 1;
                            self.failed += usize::from(result.is_err());
                            1
                        }
                        Message::OutlineParseCompleted(_) => {
                            self.outline += 1;
                            2
                        }
                        Message::SettingsPersisted(result) => {
                            self.persisted += 1;
                            self.persist_failed += usize::from(result.is_err());
                            3
                        }
                        _ => 3,
                    };
                    let start = Instant::now();
                    let task = self.app.update(message);
                    let elapsed = start.elapsed().as_micros();
                    self.handler_us[category] += elapsed;
                    self.max_handler_us[category] = self.max_handler_us[category].max(elapsed);
                    if self.completed == self.paths.len() && self.done.is_none() {
                        println!(
                            "all_loaded_ms={:.2} failed={} chunks={} handlers_us={:?} max_handler_us={:?}",
                            self.loading.unwrap().elapsed().as_secs_f64() * 1000.0,
                            self.failed,
                            self.chunks,
                            self.handler_us,
                            self.max_handler_us
                        );
                        self.done = Some(Instant::now());
                    }
                    task.map(Probe::App)
                }
                Probe::Tick => {
                    self.max_tick_us = self.max_tick_us.max(self.last_tick.elapsed().as_micros());
                    self.last_tick = Instant::now();
                    if self
                        .done
                        .is_some_and(|t| t.elapsed() > Duration::from_secs(1))
                        || self.started.elapsed() > Duration::from_secs(60)
                    {
                        println!(
                            "DONE files={} saved={} save_errors={} outlines={} max_tick_ms={:.2} total_ms={:.2}",
                            self.completed,
                            self.persisted,
                            self.persist_failed,
                            self.outline,
                            self.max_tick_us as f64 / 1000.0,
                            self.started.elapsed().as_secs_f64() * 1000.0
                        );
                        iced::exit()
                    } else {
                        Task::none()
                    }
                }
            }
        }
    }

    fn model() {
        use iced::advanced::{Layout, Renderer as _, layout, mouse, renderer, widget::Tree};
        for kind in ["text4", "text64", "rust64", "rust900", "rust1100"] {
            for count in [1, 10, 100] {
                let text = fixture(kind);
                let (mut app, _) = App::new();
                let start = Instant::now();
                for i in 0..count {
                    let _ = app.update(Message::FileOpened(Ok(OpenedFile {
                        path: format!(
                            "file_{i:03}.{}",
                            if kind.starts_with("rust") {
                                "rs"
                            } else {
                                "txt"
                            }
                        )
                        .into(),
                        contents: Arc::new(DecodedText {
                            text: text.clone(),
                            encoding: TextEncoding::Utf8,
                            had_errors: false,
                        }),
                    })));
                }
                let load = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                let _ = app.update(Message::ToggleIndentationGuides);
                let settings = start.elapsed().as_secs_f64() * 1000.0;
                let mut renderer = iced::Renderer::new(renderer::Settings::default());
                let size = iced::Size::new(1200.0, 720.0);
                let bounds = iced::Rectangle::with_size(size);
                let mut tree = None;
                let mut times = [0u128; 3];
                for _ in 0..20 {
                    let start = Instant::now();
                    let mut element = app.view(window::Id::unique());
                    times[0] += start.elapsed().as_micros();
                    let start = Instant::now();
                    let state = tree.get_or_insert_with(|| Tree::new(element.as_widget()));
                    state.diff(element.as_widget_mut());
                    let node = element.as_widget_mut().layout(
                        state,
                        &renderer,
                        &layout::Limits::new(size, size),
                    );
                    times[1] += start.elapsed().as_micros();
                    let start = Instant::now();
                    renderer.reset(bounds);
                    element.as_widget().draw(
                        state,
                        &mut renderer,
                        &iced::Theme::Light,
                        &renderer::Style {
                            text_color: iced::Color::BLACK,
                        },
                        Layout::new(&node),
                        mouse::Cursor::Unavailable,
                        &bounds,
                    );
                    times[2] += start.elapsed().as_micros();
                }
                println!(
                    "MODEL kind={kind} n={count} bytes={} handler_open_ms={load:.2} decoration_toggle_ms={settings:.2} avg_view_us={} avg_layout_us={} avg_record_us={}",
                    text.len(),
                    times[0] / 20,
                    times[1] / 20,
                    times[2] / 20
                );
            }
        }
    }

    fn history() {
        let _ = isolated_directory();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
        for count in [0,16,100] {
            let mut settings=EditorSettings::default();
            for i in 0..count { settings.record_open_history_path(format!("C:/fixtures/file_{i}.txt")); }
            let xml=settings.to_xml_string();
            let start=Instant::now();
            for _ in 0..1000 { std::hint::black_box(EditorSettings::from_xml_str(&xml)); }
            println!("HISTORY requested={count} retained={} xml_bytes={} parse_avg_us={:.2}",settings.open_history.len(),xml.len(),start.elapsed().as_secs_f64()*1000.0);
        }
        for concurrent in [false,true] {
            for count in [1,10,100] {
                let mut settings=EditorSettings::default(); let mut versions=Vec::new();
                for i in 0..count { settings.record_open_history_path(format!("C:/fixtures/file_{i:03}.txt")); versions.push(settings.clone()); }
                let start=Instant::now();
                let results=if concurrent { futures::future::join_all(versions.into_iter().map(services::save_settings)).await }
                    else { let mut r=Vec::new(); for s in versions {r.push(services::save_settings(s).await);} r };
                let elapsed=start.elapsed().as_secs_f64()*1000.0;
                let stored=services::load_settings().await.unwrap().unwrap();
                println!("HISTORY_WRITE concurrent={concurrent} n={count} ms={elapsed:.2} errors={} latest_preserved={} retained={} newest={:?}",results.iter().filter(|r|r.is_err()).count(),stored.open_history==settings.open_history,stored.open_history.len(),stored.open_history.first());
            }
        }
    });
    }

    fn outline() {
        use fragile_notepad::editor::{
            outline::{OutlineParseRequest, parse_outline_snapshot},
            outline_registry_hash,
        };
        let hash = outline_registry_hash();
        for count in [900, 1800, 3600, 7200] {
            let text = "fn sample() {\n    let value = 123;\n}\n".repeat(count);
            let start = Instant::now();
            let result = parse_outline_snapshot(OutlineParseRequest::new(
                DocumentId::new(1),
                Arc::new(text),
                "rs",
                0,
                hash,
            ));
            println!(
                "OUTLINE declarations={count} detected={} ms={:.2}",
                result.functions.len(),
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    fn recovery(verify: bool) -> iced::Result {
        use fragile_notepad::core::{Session, SessionDocument};
        let dir = isolated_directory();
        std::fs::create_dir_all(dir.join("recovery-fixtures")).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        if !verify {
            let mut entries = Vec::new();
            for i in 0..100 {
                let path = dir.join("recovery-fixtures").join(format!("file-{i}.rs"));
                std::fs::write(&path, fixture("rust64")).unwrap();
                entries.push(SessionDocument {
                    path: Some(path),
                    is_pinned: i < 2,
                    ..Default::default()
                });
            }
            entries.push(SessionDocument {
                text: Some("unsaved recovery draft 二🙂".into()),
                is_dirty: true,
                ..Default::default()
            });
            rt.block_on(services::session_store::save_session(Session {
                documents: entries,
                active_index: 100,
                ..Default::default()
            }))
            .unwrap();
        }
        let started = Instant::now();
        iced::daemon(
            move || {
                let (app, task) =
                    App::new_with_options(fragile_notepad::startup::StartupOptions::default());
                (
                    Recovery {
                        app,
                        verify,
                        loaded: 0,
                        viewed: Cell::new(false),
                        started,
                        closed: false,
                    },
                    Task::batch([
                        task.map(RecoveryMessage::App),
                        Task::perform(
                            async {
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            },
                            |_| RecoveryMessage::Close,
                        ),
                    ]),
                )
            },
            Recovery::update,
            Recovery::view,
        )
        .subscription(|state| state.app.subscription().map(RecoveryMessage::App))
        .settings(fragile_notepad::startup::iced_settings())
        .run()?;
        let saved = rt
            .block_on(services::session_store::load_session())
            .unwrap()
            .unwrap();
        assert_eq!(saved.documents.len(), 101);
        assert_eq!(saved.active_index, 100);
        assert_eq!(
            saved.documents[100].text.as_deref(),
            Some("edited recovery text 二🙂")
        );
        assert!(saved.documents[100].is_dirty);
        assert!(saved.documents[0].is_pinned && saved.documents[1].is_pinned);
        println!(
            "RECOVERY verify={verify} tabs=101 active=100 unsaved_text_exact=true elapsed_ms={:.2}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    }
    #[derive(Debug, Clone)]
    enum RecoveryMessage {
        App(Message),
        Close,
    }
    struct Recovery {
        app: App,
        verify: bool,
        loaded: usize,
        viewed: Cell<bool>,
        started: Instant,
        closed: bool,
    }
    impl Recovery {
        fn view(&self, id: window::Id) -> Element<'_, RecoveryMessage> {
            if !self.viewed.replace(true) {
                println!(
                    "RECOVERY first_view_ms={:.2}",
                    self.started.elapsed().as_secs_f64() * 1000.0
                );
            }
            self.app.view(id).map(RecoveryMessage::App)
        }
        fn update(&mut self, message: RecoveryMessage) -> Task<RecoveryMessage> {
            match message {
                RecoveryMessage::App(message) => {
                    if matches!(message, Message::FileLoadFinished(_)) {
                        self.loaded += 1;
                    }
                    if let Message::ShutdownPersisted(ref result) = message {
                        assert!(result.is_ok(), "{result:?}");
                        println!("RECOVERY shutdown_saved=true disk_loads={}", self.loaded);
                    }
                    self.app.update(message).map(RecoveryMessage::App)
                }
                RecoveryMessage::Close => {
                    assert!(!self.closed);
                    self.closed = true;
                    if !self.verify {
                        let _ = self
                            .app
                            .update(Message::FindQueryChanged("unsaved recovery draft".into()));
                        let _ = self.app.update(Message::FindReplacementChanged(
                            "edited recovery text".into(),
                        ));
                        let _ = self.app.update(Message::ReplaceAll);
                    }
                    window::oldest().map(|id| {
                        RecoveryMessage::App(Message::WindowCloseRequested(
                            id.expect("main window"),
                        ))
                    })
                }
            }
        }
    }
}
#[cfg(not(feature = "hybrid-rendering"))]
fn main() -> iced::Result {
    profile::run()
}
#[cfg(feature = "hybrid-rendering")]
fn main() {
    eprintln!(
        "Run with --no-default-features and an isolated APPDATA under target/many-files-investigation"
    );
}
