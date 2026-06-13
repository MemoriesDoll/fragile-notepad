#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use fragile_notepad::{
    app::{self, App},
    ipc::{self, SingleInstanceConfig},
    startup,
};

fn main() -> iced::Result {
    if startup::startup_probe_enabled() {
        startup::mark_startup_started();
    }

    let single_instance = SingleInstanceConfig::new("fragile-notepad");

    match ipc::claim_or_signal(&single_instance).map_err(|error| {
        iced::Error::WindowCreationFailed(Box::new(std::io::Error::new(
            error.kind(),
            format!("single-instance IPC initialization failed: {error}"),
        )))
    })? {
        ipc::Startup::Primary(instance) => app::register_single_instance(instance),
        ipc::Startup::Secondary => return Ok(()),
    }

    iced::daemon(App::new, App::update, App::view)
        .settings(startup::iced_settings())
        .subscription(App::subscription)
        .title(App::title)
        .theme(App::theme)
        .run()
}
