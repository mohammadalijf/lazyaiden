//! `login`, `logout` and `completions` through `execute`, with in-memory credentials.

use std::collections::HashMap;

use clap::Parser;
use lazyaiden_cli::{Cli, CliError, Io, default_editor, execute};
use lazyaiden_core::{CredentialStore, MemoryCredentials};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Run {
    result: Result<(), CliError>,
    out: String,
    err: String,
}

async fn exec(args: &[&str], stdin: &str, env: &[(&str, &str)], creds: &MemoryCredentials) -> Run {
    let cli =
        Cli::try_parse_from(std::iter::once("lazyaiden-cli").chain(args.iter().copied())).unwrap();
    let env: HashMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    let lookup = move |k: &str| env.get(k).cloned();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input = stdin.as_bytes();
    let mut io = Io {
        out: &mut out,
        err: &mut err,
        stdin: &mut input,
        editor: &default_editor,
    };
    let result = execute(cli, &mut io, &lookup, creds).await;
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

async fn auth_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(
            json!({"email": "me@example.com", "password": "s3cret"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"accessToken": "t", "refreshToken": "r"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn login_with_password_from_stdin_stores_verified_credentials() {
    let server = auth_server().await;
    let creds = MemoryCredentials::new();
    let home = tempfile::tempdir().unwrap();
    let env = [("HOME", home.path().to_str().unwrap())];

    let r = exec(
        &[
            "--base-url",
            &server.uri(),
            "login",
            "--email",
            "me@example.com",
            "--password-stdin",
        ],
        "s3cret\n",
        &env,
        &creds,
    )
    .await;
    r.result.unwrap();
    assert_eq!(r.out, "Logged in as me@example.com; credentials saved.\n");
    assert_eq!(creds.load().unwrap().unwrap().password, "s3cret");
    assert!(!r.out.contains("s3cret") && !r.err.contains("s3cret"));

    let r = exec(&["logout"], "", &env, &creds).await;
    r.result.unwrap();
    assert!(creds.load().unwrap().is_none());
}

#[tokio::test]
async fn wrong_password_is_rejected_with_exit_3_and_nothing_stored() {
    let server = auth_server().await;
    let creds = MemoryCredentials::new();
    let home = tempfile::tempdir().unwrap();
    let r = exec(
        &[
            "--base-url",
            &server.uri(),
            "login",
            "--email",
            "me@example.com",
            "--password-stdin",
        ],
        "wrong\n",
        &[("HOME", home.path().to_str().unwrap())],
        &creds,
    )
    .await;
    assert_eq!(r.result.unwrap_err().exit_code(), 3);
    assert!(creds.load().unwrap().is_none());
}

#[tokio::test]
async fn login_input_validation() {
    let creds = MemoryCredentials::new();
    let home = tempfile::tempdir().unwrap();
    let env = [("HOME", home.path().to_str().unwrap())];

    let r = exec(&["login", "--password-stdin"], "pw\n", &env, &creds).await;
    assert_eq!(r.result.unwrap_err().exit_code(), 2, "needs an email");

    let r = exec(
        &["login", "--email", "a@b.c", "--password-stdin"],
        "\n",
        &env,
        &creds,
    )
    .await;
    assert_eq!(r.result.unwrap_err().exit_code(), 2, "empty password");
}

#[tokio::test]
async fn email_may_come_from_the_environment() {
    let server = auth_server().await;
    let creds = MemoryCredentials::new();
    let home = tempfile::tempdir().unwrap();
    let r = exec(
        &["--base-url", &server.uri(), "login", "--password-stdin"],
        "s3cret",
        &[
            ("HOME", home.path().to_str().unwrap()),
            ("FELLOW_EMAIL", "me@example.com"),
        ],
        &creds,
    )
    .await;
    r.result.unwrap();
    assert_eq!(creds.load().unwrap().unwrap().email, "me@example.com");
}

#[tokio::test]
async fn completions_for_every_shell_are_nonempty() {
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let r = exec(&["completions", shell], "", &[], &MemoryCredentials::new()).await;
        r.result.unwrap();
        assert!(r.out.contains("lazyaiden-cli"), "{shell}");
        assert_eq!(r.err, "");
    }
}
