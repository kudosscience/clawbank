//! CLI seam: `clawbank init` behavior through the real binary.
//! Each test owns an isolated home dir via CLAWBANK_HOME on the child
//! process only, so tests never touch the real profile and never race.

use std::process::Command;

fn home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn init(home: &tempfile::TempDir) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_clawbank"))
        .env("CLAWBANK_HOME", home.path())
        .arg("init")
        .output()
        .unwrap();
    (out.status.success(), String::from_utf8(out.stdout).unwrap())
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
fn init_twice_prints_the_same_peer_id() {
    let home = home();
    let (_, first) = init(&home);
    let (ok, second) = init(&home);
    assert!(ok);
    assert_eq!(peer_ids(&first), peer_ids(&second));
}

#[test]
fn init_after_delete_prints_a_different_peer_id() {
    let home = home();
    let (_, first) = init(&home);
    std::fs::remove_file(home.path().join("identity.key")).unwrap();
    let (ok, second) = init(&home);
    assert!(ok);
    assert_ne!(peer_ids(&first), peer_ids(&second));
}

#[test]
fn init_prints_parseable_base58_and_cid_forms() {
    let home = home();
    let (ok, out) = init(&home);
    assert!(ok);
    let (base58, cid) = peer_ids(&out);
    assert!(base58.starts_with("12D3Koo"), "unexpected base58: {base58}");
    assert!(cid.starts_with('b'), "unexpected CID: {cid}");
}

#[test]
fn init_creates_the_identity_file_under_home() {
    let home = home();
    let (ok, _) = init(&home);
    assert!(ok);
    assert!(home.path().join("identity.key").is_file());
}

#[test]
fn concurrent_init_processes_converge_on_the_stored_identity() {
    use std::process::Command;
    let home = home();
    // Separate OS processes (not threads) so the inter-process first-run
    // lock is genuinely exercised.
    let children: Vec<_> = (0..8)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_clawbank"))
                .env("CLAWBANK_HOME", home.path())
                .arg("init")
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut ids = std::collections::HashSet::new();
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        ids.insert(peer_ids(&stdout));
    }
    assert_eq!(ids.len(), 1, "all processes must report one identity");
}
