use fragile_notepad::startup::{StartupCommand, parse_arguments};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn cli_resolves_paths_and_separator_without_requiring_files_to_exist() {
    let cwd = std::env::current_dir().unwrap();
    let command = parse_arguments(
        ["--no-session", "notes/draft.txt", "--", "-new.txt"].map(OsString::from),
        &cwd,
    )
    .unwrap();
    let StartupCommand::Launch(options) = command else {
        panic!("launch");
    };
    assert!(!options.restore_session);
    assert_eq!(
        options.files,
        vec![cwd.join("notes/draft.txt"), cwd.join("-new.txt")]
    );
    assert!(parse_arguments([OsString::from("--unknown")], &cwd).is_err());
}

#[test]
fn cli_help_and_version_exit_without_starting_gui() {
    for (argument, expected) in [
        ("--help", "Usage: fragile-notepad"),
        ("--version", env!("CARGO_PKG_VERSION")),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_fragile-notepad"))
            .arg(argument)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
    }
}

#[test]
fn cli_preserves_non_unicode_native_path() {
    #[cfg(windows)]
    let name = {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0xd800, 46, 116, 120, 116])
    };
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![0xff, b'.', b't', b'x', b't'])
    };
    #[cfg(not(any(windows, unix)))]
    let name = OsString::from("native.txt");
    let cwd = std::env::current_dir().unwrap();
    let StartupCommand::Launch(options) = parse_arguments([name.clone()], &cwd).unwrap() else {
        panic!("launch");
    };
    assert_eq!(options.files, vec![cwd.join(name)]);
}

#[test]
fn ipc_child_sender() {
    let Some(id) = std::env::var_os("FRAGILE_CLI_TEST_INSTANCE") else {
        return;
    };
    let config = fragile_notepad::ipc::SingleInstanceConfig::new(id.to_str().unwrap());
    let file = PathBuf::from(std::env::var_os("FRAGILE_CLI_TEST_FILE").unwrap());
    let result = fragile_notepad::ipc::claim_or_signal_with_files(&config, &[file]);
    if std::env::var_os("FRAGILE_CLI_TEST_REJECT").is_some() {
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::ConnectionAborted
        );
    } else {
        assert!(matches!(
            result.unwrap(),
            fragile_notepad::ipc::Startup::Secondary
        ));
    }
}

#[cfg(any(windows, unix))]
#[test]
fn secondary_process_observes_application_rejection() {
    use fragile_notepad::ipc::{self, SingleInstanceConfig, Startup};
    let id = format!("cli-rejection-test-{}", std::process::id());
    let config = SingleInstanceConfig::new(&id);
    let Startup::Primary(primary) = ipc::claim_or_signal(&config).unwrap() else {
        panic!("primary");
    };
    let receiver = std::thread::spawn(move || primary.accept_signal_with(|_| false));
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "ipc_child_sender", "--nocapture"])
        .env("FRAGILE_CLI_TEST_INSTANCE", &id)
        .env(
            "FRAGILE_CLI_TEST_FILE",
            std::env::temp_dir().join("rejected.txt"),
        )
        .env("FRAGILE_CLI_TEST_REJECT", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    receiver.join().unwrap().unwrap();
}

#[cfg(any(windows, unix))]
#[test]
fn secondary_process_delivers_files_to_primary_instance() {
    use fragile_notepad::ipc::{self, ActivationRequest, Signal, SingleInstanceConfig, Startup};
    let id = format!("cli-process-test-{}", std::process::id());
    let config = SingleInstanceConfig::new(&id);
    let Startup::Primary(primary) = ipc::claim_or_signal(&config).unwrap() else {
        panic!("primary");
    };
    let expected = std::env::temp_dir().join("forwarded file.txt");
    let receiver = std::thread::spawn(move || primary.accept_signal());
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "ipc_child_sender", "--nocapture"])
        .env("FRAGILE_CLI_TEST_INSTANCE", &id)
        .env("FRAGILE_CLI_TEST_FILE", &expected)
        .env_remove("XDG_ACTIVATION_TOKEN")
        .env_remove("DESKTOP_STARTUP_ID")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        receiver.join().unwrap().unwrap(),
        Signal::OpenFiles(vec![expected], ActivationRequest::default())
    );
}
