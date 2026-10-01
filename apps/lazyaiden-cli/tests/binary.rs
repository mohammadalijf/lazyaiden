//! Tests that spawn the real `lazyaiden-cli` binary.
//!
//! Dummy FELLOW_* variables are always set so the OS keychain is never consulted.

use assert_cmd::Command;
use predicates::prelude::*;

const MORNING: &str = include_str!("../../../examples/profiles/morning-v60.yaml");

fn cli(dir: &tempfile::TempDir) -> Command {
    let mut c = Command::cargo_bin("lazyaiden-cli").unwrap();
    c.env_clear()
        .env("HOME", dir.path())
        .env("FELLOW_EMAIL", "test@example.com")
        .env("FELLOW_PASSWORD", "not-a-real-password")
        .args(["--config"])
        .arg(dir.path().join("config.toml"))
        .arg("--profiles-dir")
        .arg(dir.path().join("profiles"));
    c
}

#[test]
fn help_and_version() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir).arg("--help").assert().success().stdout(
        predicate::str::contains("profile")
            .and(predicate::str::contains("schedule"))
            .and(predicate::str::contains("device")),
    );
    cli(&dir)
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("lazyaiden-cli"));
    cli(&dir)
        .args(["profile", "add", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--input").and(predicate::str::contains("--overwrite")));
}

#[test]
fn usage_errors_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir)
        .arg("frobnicate")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unrecognized"));
    cli(&dir).args(["profile", "add"]).assert().code(2);
    cli(&dir)
        .args(["schedule", "add", "--water", "lots"])
        .assert()
        .code(2);
}

#[test]
fn headless_import_list_export_round_trip_without_network() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("profile.yaml");
    std::fs::write(&file, MORNING).unwrap();

    cli(&dir)
        .args(["profile", "add", "-i"])
        .arg(&file)
        .assert()
        .success()
        .stdout("Imported \"Morning V60\" as morning-v60.\n");

    cli(&dir)
        .args(["--json", "profile", "list", "--local"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"morning-v60\""));

    cli(&dir)
        .args(["profile", "export", "Morning V60"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bloomDuration: 35"));
}

#[test]
fn import_from_stdin() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir)
        .args(["profile", "add", "-i", "-"])
        .write_stdin(MORNING)
        .assert()
        .success();
    assert!(dir.path().join("profiles/morning-v60.yaml").is_file());
}

#[test]
fn invalid_profile_exits_4_and_names_the_field() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir)
        .args(["profile", "add", "-i", "-"])
        .write_stdin(MORNING.replace("ratio: 16.0", "ratio: 99"))
        .assert()
        .code(4)
        .stderr(predicate::str::contains("ratio"));
    assert!(
        !dir.path().join("profiles").exists()
            || std::fs::read_dir(dir.path().join("profiles"))
                .unwrap()
                .count()
                == 0
    );
}

#[test]
fn missing_profile_exits_5_and_missing_file_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir).args(["profile", "show", "nope"]).assert().code(5);
    cli(&dir)
        .args(["profile", "add", "-i", "/definitely/not/here.yaml"])
        .assert()
        .code(1);
}

#[test]
fn demo_mode_runs_offline_with_sample_data() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir)
        .args(["--demo", "profile", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Demo Everyday").and(predicate::str::contains("remote-only")),
        );
    cli(&dir)
        .args(["--demo", "device", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("demo-brewer"));
}

#[test]
fn remote_commands_without_credentials_exit_3_with_a_login_hint() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Command::cargo_bin("lazyaiden-cli").unwrap();
    // No FELLOW_* variables; HOME points at an empty directory so no keychain entry can exist.
    c.env_clear()
        .env("HOME", dir.path())
        .env("XDG_RUNTIME_DIR", dir.path())
        .args(["--config"])
        .arg(dir.path().join("config.toml"))
        .args(["profile", "list"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("not logged in"));
}

#[test]
fn completions_are_generated() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir)
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("lazyaiden-cli"));
    cli(&dir).args(["completions", "klingon"]).assert().code(2);
}

#[test]
fn man_pages_are_printed_and_installed() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir).arg("man").assert().success().stdout(
        predicate::str::contains(".TH lazyaiden-cli 1").and(predicate::str::contains("profile")),
    );

    let out = dir.path().join("man/man1");
    cli(&dir)
        .args(["man", "--dir"])
        .arg(&out)
        .assert()
        .success();
    for page in [
        "lazyaiden-cli.1",
        "lazyaiden-cli-profile.1",
        "lazyaiden-cli-profile-push.1",
    ] {
        assert!(out.join(page).is_file(), "missing {page}");
    }
}

#[test]
fn unreachable_server_exits_7() {
    let dir = tempfile::tempdir().unwrap();
    cli(&dir)
        .args(["--base-url", "http://127.0.0.1:1", "device", "list"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("network error"));
}
