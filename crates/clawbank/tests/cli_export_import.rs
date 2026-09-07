//! CLI seam: `clawbank export` / `clawbank import` backup and restore.
//! Each test owns an isolated home dir via CLAWBANK_HOME on the child
//! process only, so tests never touch the real profile and never race.

use std::process::{Command, Stdio};

fn home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

struct CliOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_cli(home: &tempfile::TempDir, args: &[&str], stdin: Option<&str>) -> CliOutput {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_clawbank"));
    cmd.env("CLAWBANK_HOME", home.path()).args(args);
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(input) = stdin {
        use std::io::Write;
        // The child may reject the input and exit before we finish
        // writing (e.g. the oversized-import test): a broken pipe then
        // means rejection already happened, not a harness failure.
        match child.stdin.as_mut().unwrap().write_all(input.as_bytes()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(e) => panic!("stdin write failed: {e}"),
        }
    }
    let out = child.wait_with_output().unwrap();
    CliOutput {
        success: out.status.success(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

fn output_value(stdout: &str, label: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(label))
        .unwrap()
        .to_string()
}

fn peer_ids(stdout: &str) -> (String, String) {
    (
        output_value(stdout, "Peer ID (base58): "),
        output_value(stdout, "Peer ID (CID): "),
    )
}

#[test]
fn export_after_init_prints_single_line_portable_material() {
    let home = home();
    assert!(run_cli(&home, &["init"], None).success);
    let out = run_cli(&home, &["export"], None);
    assert!(out.success, "export must succeed: {}", out.stdout);
    let line = out.stdout.trim().to_string();
    assert!(!line.is_empty());
    assert!(
        !line.contains(char::is_whitespace),
        "export must be one line"
    );
}

#[test]
fn delete_then_import_restores_identical_peer_id() {
    let home = home();
    let init_out = run_cli(&home, &["init"], None);
    let before = peer_ids(&init_out.stdout);
    let export_out = run_cli(&home, &["export"], None);
    let export_text = export_out.stdout.trim().to_string();
    let file = home.path().join("identity.key");
    let before_bytes = std::fs::read(&file).unwrap();

    std::fs::remove_file(&file).unwrap();
    let import_out = run_cli(&home, &["import", export_text.as_str()], None);
    assert!(
        import_out.success,
        "import must succeed: {}",
        import_out.stdout
    );
    assert_eq!(peer_ids(&import_out.stdout), before);
    assert_eq!(std::fs::read(&file).unwrap(), before_bytes);
}

#[test]
fn losing_file_without_export_loses_peer_id() {
    let home = home();
    let first = run_cli(&home, &["init"], None);
    assert!(first.success);
    let before = peer_ids(&first.stdout);
    std::fs::remove_file(home.path().join("identity.key")).unwrap();
    // No export kept: re-init cannot recover, it mints a fresh PeerId.
    let second = run_cli(&home, &["init"], None);
    assert!(second.success);
    assert_ne!(
        peer_ids(&second.stdout),
        before,
        "losing both file and export must lose the PeerId"
    );
}

#[test]
fn import_via_stdin_restores_identical_peer_id() {
    let home = home();
    let init_out = run_cli(&home, &["init"], None);
    let before = peer_ids(&init_out.stdout);
    let export_out = run_cli(&home, &["export"], None);
    std::fs::remove_file(home.path().join("identity.key")).unwrap();
    let import_out = run_cli(&home, &["import"], Some(export_out.stdout.trim()));
    assert!(
        import_out.success,
        "stdin import must succeed: {}",
        import_out.stdout
    );
    assert_eq!(peer_ids(&import_out.stdout), before);
}

#[test]
fn import_malformed_fails_and_leaves_state_untouched() {
    let home = home();
    let init_out = run_cli(&home, &["init"], None);
    let before = peer_ids(&init_out.stdout);
    let file = home.path().join("identity.key");
    let before_bytes = std::fs::read(&file).unwrap();

    for bad in ["!!!not-base64!!!", "aGVsbG8td29ybGQ="] {
        let out = run_cli(&home, &["import", bad], None);
        assert!(!out.success, "malformed import must fail: {bad}");
        assert!(
            out.stderr.contains("not a valid identity export"),
            "clear error, got: {}",
            out.stderr
        );
    }
    assert_eq!(std::fs::read(&file).unwrap(), before_bytes);
    let second_init = run_cli(&home, &["init"], None);
    assert_eq!(peer_ids(&second_init.stdout), before);
}

#[test]
fn import_oversized_stdin_fails_without_touching_state() {
    let home = home();
    let init_out = run_cli(&home, &["init"], None);
    assert!(init_out.success);
    let before = peer_ids(&init_out.stdout);
    let file = home.path().join("identity.key");
    let before_bytes = std::fs::read(&file).unwrap();
    // Far beyond any valid export (~100 bytes): must be rejected by size,
    // not merely by later base64/protobuf validation.
    let huge = "A".repeat(128 * 1024);
    let out = run_cli(&home, &["import"], Some(huge.as_str()));
    assert!(!out.success);
    assert!(
        out.stderr.contains("too large"),
        "oversized input needs a size error, got: {}",
        out.stderr
    );
    assert_eq!(std::fs::read(&file).unwrap(), before_bytes);
    let second_init = run_cli(&home, &["init"], None);
    assert_eq!(peer_ids(&second_init.stdout), before);
}

#[test]
fn import_malformed_with_no_identity_creates_nothing() {
    let home = home();
    let out = run_cli(&home, &["import", "!!!not-base64!!!"], None);
    assert!(!out.success);
    assert!(
        out.stderr.contains("not a valid identity export"),
        "got: {}",
        out.stderr
    );
    assert!(!home.path().join("identity.key").exists());
}

#[test]
fn help_documents_loss_of_file_and_export_loses_peer_id() {
    let home = home();
    let export_help = run_cli(&home, &["export", "--help"], None);
    assert!(
        export_help.stdout.contains("unrecoverable by design"),
        "export help must warn: {}",
        export_help.stdout
    );
    let import_help = run_cli(&home, &["import", "--help"], None);
    assert!(
        import_help.stdout.contains("unrecoverable by design"),
        "import help must warn: {}",
        import_help.stdout
    );
}
