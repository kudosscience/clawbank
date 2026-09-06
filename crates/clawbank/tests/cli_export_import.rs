//! CLI seam: `clawbank export` / `clawbank import` backup and restore.
//! Each test owns an isolated home dir via CLAWBANK_HOME on the child
//! process only, so tests never touch the real profile and never race.

use std::process::{Command, Stdio};

fn home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn run(home: &tempfile::TempDir, args: &[&str], stdin: Option<&str>) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_clawbank"));
    cmd.env("CLAWBANK_HOME", home.path()).args(args);
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    if let Some(input) = stdin {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.success(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
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
    assert!(run(&home, &["init"], None).0);
    let (ok, out, _) = run(&home, &["export"], None);
    assert!(ok, "export must succeed, stderr+stdout: {out}");
    let line = out.trim().to_string();
    assert!(!line.is_empty());
    assert!(!line.contains(char::is_whitespace), "export must be one line");
}

#[test]
fn delete_then_import_restores_identical_peer_id() {
    let home = home();
    let (_, init_out, _) = run(&home, &["init"], None);
    let before = peer_ids(&init_out);
    let (_, export_out, _) = run(&home, &["export"], None);
    let export_text = export_out.trim().to_string();
    let file = home.path().join("identity.key");
    let before_bytes = std::fs::read(&file).unwrap();

    std::fs::remove_file(&file).unwrap();
    let (ok, import_out, _) = run(&home, &["import", export_text.as_str()], None);
    assert!(ok, "import must succeed: {import_out}");
    assert_eq!(peer_ids(&import_out), before);
    assert_eq!(std::fs::read(&file).unwrap(), before_bytes);
}

#[test]
fn import_via_stdin_restores_identical_peer_id() {
    let home = home();
    let (_, init_out, _) = run(&home, &["init"], None);
    let before = peer_ids(&init_out);
    let (_, export_out, _) = run(&home, &["export"], None);
    std::fs::remove_file(home.path().join("identity.key")).unwrap();
    let (ok, import_out, _) = run(&home, &["import"], Some(export_out.trim()));
    assert!(ok, "stdin import must succeed: {import_out}");
    assert_eq!(peer_ids(&import_out), before);
}

#[test]
fn import_malformed_fails_and_leaves_state_untouched() {
    let home = home();
    let (_, init_out, _) = run(&home, &["init"], None);
    let before = peer_ids(&init_out);
    let file = home.path().join("identity.key");
    let before_bytes = std::fs::read(&file).unwrap();

    for bad in ["!!!not-base64!!!", "aGVsbG8td29ybGQ="] {
        let (ok, _, err) = run(&home, &["import", bad], None);
        assert!(!ok, "malformed import must fail: {bad}");
        assert!(
            err.contains("not a valid identity export"),
            "clear error, got: {err}"
        );
    }
    assert_eq!(std::fs::read(&file).unwrap(), before_bytes);
    let (_, second_init, _) = run(&home, &["init"], None);
    assert_eq!(peer_ids(&second_init), before);
}

#[test]
fn import_malformed_with_no_identity_creates_nothing() {
    let home = home();
    let (ok, _, err) = run(&home, &["import", "!!!not-base64!!!"], None);
    assert!(!ok);
    assert!(err.contains("not a valid identity export"), "got: {err}");
    assert!(!home.path().join("identity.key").exists());
}

#[test]
fn help_documents_loss_of_file_and_export_loses_peer_id() {
    let home = home();
    let (_, help_out, _) = run(&home, &["export", "--help"], None);
    assert!(
        help_out.contains("unrecoverable by design"),
        "export help must warn: {help_out}"
    );
    let (_, help_out, _) = run(&home, &["import", "--help"], None);
    assert!(
        help_out.contains("unrecoverable by design"),
        "import help must warn: {help_out}"
    );
}
