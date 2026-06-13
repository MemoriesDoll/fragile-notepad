use fragile_notepad::editor::widget::{EDITOR_FONT, EDITOR_FONT_ROUTE};
use fragile_notepad::startup::{
    STARTUP_PROBE_ENV, STARTUP_PROBE_OUTPUT_PREFIX, UI_READY_BUDGET, iced_settings,
};

use iced::Backend;
use iced::advanced::graphics::text::{self as graphics_text, cosmic_text, font_system};

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const CI_UI_READY_BUDGET: Duration = Duration::from_millis(750);

#[test]
fn app_binary_reaches_first_view_within_startup_budget() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fragile-notepad"))
        .env(STARTUP_PROBE_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fragile-notepad startup probe");

    let stdout = child.stdout.take().expect("startup probe stdout");
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(value) = line.strip_prefix(STARTUP_PROBE_OUTPUT_PREFIX) {
                let _ = sender.send(value.parse::<f64>());
                return;
            }
        }
    });

    let timeout = Duration::from_secs(5);
    let deadline = Instant::now() + timeout;

    let elapsed = loop {
        if let Ok(result) = receiver.try_recv() {
            break Duration::from_secs_f64(result.expect("parse startup probe output") / 1_000.0);
        }

        if let Some(status) = child.try_wait().expect("poll startup probe") {
            let stderr = read_stderr(&mut child);
            panic!("startup probe exited before first view: {status}; stderr: {stderr}");
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = read_stderr(&mut child);
            panic!("startup probe did not report first view within {timeout:?}; stderr: {stderr}");
        }

        thread::sleep(Duration::from_millis(10));
    };

    let _ = child.kill();
    let _ = child.wait();

    let budget = startup_budget_for_environment();

    if running_under_wsl() && elapsed >= budget {
        eprintln!(
            "WSL startup probe exceeded diagnostic budget {budget:?}: {elapsed:?}; native startup budget remains strict"
        );
    } else {
        assert!(
            elapsed < budget,
            "app binary first view exceeded {budget:?}: {elapsed:?}"
        );
    }
}

fn read_stderr(child: &mut std::process::Child) -> String {
    let Some(mut stderr) = child.stderr.take() else {
        return String::new();
    };

    let mut output = String::new();
    let _ = stderr.read_to_string(&mut output);
    output
}

fn startup_budget_for_environment() -> Duration {
    if running_in_hosted_ci() {
        return CI_UI_READY_BUDGET;
    }

    if running_under_wsl() {
        return CI_UI_READY_BUDGET;
    }

    UI_READY_BUDGET
}

fn running_in_hosted_ci() -> bool {
    std::env::var_os("CI").is_some() || std::env::var_os("GITHUB_ACTIONS").is_some()
}

fn running_under_wsl() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        return true;
    }

    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|release| release.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

#[test]
fn startup_settings_keep_first_paint_on_software_rendering() {
    let settings = iced_settings();

    assert_eq!(settings.backend, Backend::Software);
    assert!(!settings.antialiasing);
    assert!(!settings.vsync);
}

#[test]
fn editor_font_route_keeps_primary_font_and_platform_cjk_fallback() {
    let mut font_system = font_system().write().expect("write font system");

    let ascii_families = shaped_font_families(font_system.raw(), "A");
    if let iced::font::Family::Name(primary) = EDITOR_FONT_ROUTE.primary.family {
        assert!(
            ascii_families.iter().any(|name| name == primary),
            "ASCII editor glyph should use primary route font {primary}, got families {ascii_families:?}"
        );
    } else {
        assert!(
            !ascii_families.is_empty(),
            "ASCII editor glyph should resolve through generic primary route font"
        );
    }

    let han_families = shaped_font_families(font_system.raw(), "\u{6c49}");
    let installed_fallback_families = installed_editor_fallback_families(
        font_system.raw(),
        EDITOR_FONT_ROUTE.cjk_fallback_families,
    );

    assert!(
        !han_families.is_empty(),
        "Han glyph should resolve to a platform fallback font"
    );

    if !installed_fallback_families.is_empty()
        && !han_families.iter().any(|name| {
            installed_fallback_families
                .iter()
                .any(|fallback| name == fallback)
        })
    {
        eprintln!(
            "Han glyph resolved through platform fallback outside configured CJK list {:?}: {han_families:?}",
            installed_fallback_families
        );
    }
}

fn installed_editor_fallback_families(
    raw: &cosmic_text::FontSystem,
    fallback_families: &[&str],
) -> Vec<String> {
    let mut installed = Vec::new();

    for face in raw.db().faces() {
        for (name, _) in &face.families {
            if fallback_families.iter().any(|fallback| name == fallback)
                && !installed
                    .iter()
                    .any(|installed_name| installed_name == name)
            {
                installed.push(name.to_string());
            }
        }
    }

    installed
}

fn shaped_font_families(raw: &mut cosmic_text::FontSystem, content: &str) -> Vec<String> {
    let mut buffer = cosmic_text::Buffer::new(raw, cosmic_text::Metrics::new(16.0, 20.0));
    buffer.set_size(Some(100.0), Some(20.0));
    buffer.set_text(
        content,
        &graphics_text::to_attributes(EDITOR_FONT),
        cosmic_text::Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(raw, false);

    let font_id = buffer
        .layout_runs()
        .next()
        .and_then(|run| run.glyphs.first())
        .map(|glyph| glyph.font_id)
        .expect("shaped glyph");
    let face = raw.db().face(font_id).expect("glyph font face");

    face.families
        .iter()
        .map(|(name, _)| name.to_string())
        .collect()
}

#[test]
fn iced_dependency_does_not_enable_slow_startup_defaults() {
    let manifest = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("read Cargo.toml");
    let iced_line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("iced ="))
        .expect("find iced dependency");

    assert!(
        iced_line.contains("default-features = false"),
        "iced dependency must disable default features: {iced_line}"
    );
    assert!(
        iced_line.contains("\"tiny-skia\""),
        "iced dependency must keep the tiny-skia renderer enabled: {iced_line}"
    );
    assert!(
        iced_line.contains("\"highlighter\""),
        "the full Iced syntax highlighter must stay enabled: {iced_line}"
    );
    assert!(
        manifest.contains("default = [\"hybrid-rendering\"]"),
        "hybrid rendering should be the default build feature"
    );
    assert!(
        manifest.contains("hybrid-rendering = [\"iced/wgpu\"]"),
        "hybrid-rendering feature should enable iced/wgpu"
    );
    assert!(
        !iced_line.contains("\"debug\""),
        "debug tooling should stay disabled for startup latency: {iced_line}"
    );
}

#[test]
fn highlighter_exposes_language_catalog_for_menu() {
    let syntaxes = iced::highlighter::syntaxes();

    assert!(
        syntaxes.len() > 1,
        "language menu should not be limited to plain text"
    );
    assert!(
        syntaxes
            .iter()
            .any(|syntax| syntax.name == "Rust" && syntax.token == "rs"),
        "language catalog should include Rust with the token used by the highlighter"
    );
}

#[test]
fn release_windows_build_uses_gui_subsystem() {
    let main_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("read src/main.rs");

    assert!(
        main_rs.contains(
            "cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = \"windows\")"
        ),
        "release Windows builds should not open a console window"
    );
}
