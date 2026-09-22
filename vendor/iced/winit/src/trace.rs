use crate::window;

use rustc_hash::FxHashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static TRACE: OnceLock<Option<Mutex<PerfTrace>>> = OnceLock::new();
static LAST_FRAME: OnceLock<Mutex<FxHashMap<window::Id, Instant>>> = OnceLock::new();

struct PerfTrace {
    writer: BufWriter<File>,
}

pub fn enabled() -> bool {
    trace().is_some()
}

pub fn event(event: &'static str, elapsed_us: u128, detail: fmt::Arguments<'_>) {
    let Some(trace) = trace() else {
        return;
    };

    let Ok(mut trace) = trace.lock() else {
        return;
    };

    let timestamp_us = timestamp_us();
    let _ = writeln!(
        trace.writer,
        "{timestamp_us},{event},{elapsed_us},{}",
        csv_escape(&detail.to_string())
    );
    // Keep phase evidence immediately observable to lifecycle probes, while
    // batching interaction/draw detail until the frame is complete.
    if event == "winit_redraw_frame" || !event.starts_with("winit_redraw_") {
        let _ = trace.writer.flush();
    }
}

pub fn frame_delta_us(window: window::Id, now: Instant) -> Option<u128> {
    let Ok(mut last_frame) = LAST_FRAME
        .get_or_init(|| Mutex::new(FxHashMap::default()))
        .lock()
    else {
        return None;
    };

    last_frame
        .insert(window, now)
        .map(|last| now.duration_since(last).as_micros())
}

fn trace() -> Option<&'static Mutex<PerfTrace>> {
    TRACE
        .get_or_init(|| {
            if std::env::var_os("FRAGILE_PERF_TRACE").is_none() {
                return None;
            }

            Some(Mutex::new(PerfTrace::new()?))
        })
        .as_ref()
}

impl PerfTrace {
    fn new() -> Option<Self> {
        let path = trace_path()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        let mut writer = BufWriter::new(file);

        let _ = writeln!(writer, "timestamp_us,event,elapsed_us,detail");
        let _ = writeln!(
            writer,
            "{},trace_start,0,{}",
            timestamp_us(),
            csv_escape(&format!("path={}", path.display()))
        );

        Some(Self { writer })
    }
}

fn trace_path() -> Option<PathBuf> {
    let dir = std::env::var_os("FRAGILE_PERF_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target").join("perf"));

    std::fs::create_dir_all(&dir).ok()?;

    Some(dir.join("fragile-perf.csv"))
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn timestamp_us() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_micros())
}
