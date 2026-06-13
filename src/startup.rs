use iced::{Backend, Settings};

use std::io::{self, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const UI_READY_BUDGET: Duration = Duration::from_millis(200);
pub const STARTUP_PROBE_ENV: &str = "FRAGILE_NOTEPAD_STARTUP_PROBE";
pub const STARTUP_PROBE_OUTPUT_PREFIX: &str = "FRAGILE_NOTEPAD_FIRST_VIEW_READY_MS=";

static STARTED_AT: OnceLock<Instant> = OnceLock::new();
static REPORTED_FIRST_VIEW: AtomicBool = AtomicBool::new(false);

pub fn iced_settings() -> Settings {
    Settings {
        backend: Backend::Software,
        antialiasing: false,
        vsync: false,
        ..Settings::default()
    }
}

pub fn startup_probe_enabled() -> bool {
    std::env::var_os(STARTUP_PROBE_ENV).is_some()
}

pub fn mark_startup_started() {
    let _ = STARTED_AT.set(Instant::now());
}

pub fn report_first_view_ready() {
    if !startup_probe_enabled() || REPORTED_FIRST_VIEW.swap(true, Ordering::Relaxed) {
        return;
    }

    let elapsed = STARTED_AT.get_or_init(Instant::now).elapsed();
    println!(
        "{STARTUP_PROBE_OUTPUT_PREFIX}{}",
        elapsed.as_secs_f64() * 1_000.0
    );
    let _ = io::stdout().flush();
}
