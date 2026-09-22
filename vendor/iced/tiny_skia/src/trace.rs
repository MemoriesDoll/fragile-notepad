use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static TRACE: OnceLock<Option<Mutex<PerfTrace>>> = OnceLock::new();

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
    // Primitive events are buffered together; present is the visibility barrier
    // used by the handoff probe. Do not perform a file flush per primitive.
    if event == "tiny_skia_present" {
        let _ = trace.writer.flush();
    }
}

pub(crate) fn flush() {
    if let Some(trace) = trace()
        && let Ok(mut trace) = trace.lock()
    {
        let _ = trace.writer.flush();
    }
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
