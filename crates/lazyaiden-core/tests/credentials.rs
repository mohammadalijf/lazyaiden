use std::collections::HashMap;

use lazyaiden_core::{
    CoreError, CredentialStore, Credentials, EnvCredentials, LayeredCredentials, MemoryCredentials,
};

fn env_creds(
    pairs: &[(&str, &str)],
) -> EnvCredentials<impl Fn(&str) -> Option<String> + Send + Sync> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    EnvCredentials::new(move |k| map.get(k).cloned().filter(|v| !v.is_empty()))
}

#[test]
fn env_needs_both_variables() {
    let both = env_creds(&[("FELLOW_EMAIL", "a@b.c"), ("FELLOW_PASSWORD", "pw")]);
    let c = both.load().unwrap().unwrap();
    assert_eq!((c.email.as_str(), c.password.as_str()), ("a@b.c", "pw"));

    assert!(env_creds(&[]).load().unwrap().is_none());
    assert!(
        matches!(env_creds(&[("FELLOW_EMAIL", "a")]).load(), Err(CoreError::Credentials(m)) if m.contains("FELLOW_PASSWORD"))
    );
    assert!(
        matches!(env_creds(&[("FELLOW_PASSWORD", "a")]).load(), Err(CoreError::Credentials(m)) if m.contains("FELLOW_EMAIL"))
    );
    assert!(
        env_creds(&[("FELLOW_EMAIL", ""), ("FELLOW_PASSWORD", "")])
            .load()
            .unwrap()
            .is_none()
    );
}

#[test]
fn env_is_read_only_but_clear_is_harmless() {
    let e = env_creds(&[]);
    assert!(e.save(&Credentials::new("a", "b")).is_err());
    assert!(e.clear().is_ok());
}

#[test]
fn memory_store_round_trips() {
    let m = MemoryCredentials::new();
    assert!(m.load().unwrap().is_none());
    m.save(&Credentials::new("a", "b")).unwrap();
    assert_eq!(m.load().unwrap().unwrap().email, "a");
    m.clear().unwrap();
    assert!(m.load().unwrap().is_none());
    assert_eq!(
        MemoryCredentials::with(Credentials::new("x", "y"))
            .load()
            .unwrap()
            .unwrap()
            .email,
        "x"
    );
}

#[test]
fn layered_reads_primary_first_and_writes_to_fallback() {
    let env = env_creds(&[("FELLOW_EMAIL", "env@x"), ("FELLOW_PASSWORD", "envpw")]);
    let keychain = MemoryCredentials::with(Credentials::new("key@x", "keypw"));
    let layered = LayeredCredentials::new(Box::new(env), Box::new(keychain));
    assert_eq!(layered.load().unwrap().unwrap().email, "env@x", "env wins");

    layered.save(&Credentials::new("new@x", "pw")).unwrap();
    layered.clear().unwrap();
    assert_eq!(
        layered.load().unwrap().unwrap().email,
        "env@x",
        "clear only touches the fallback"
    );
}

#[test]
fn layered_falls_back_when_primary_is_empty() {
    let layered = LayeredCredentials::new(
        Box::new(env_creds(&[])),
        Box::new(MemoryCredentials::with(Credentials::new("key@x", "pw"))),
    );
    assert_eq!(layered.load().unwrap().unwrap().email, "key@x");
    layered.clear().unwrap();
    assert!(layered.load().unwrap().is_none());
}

#[test]
fn layered_surfaces_a_half_configured_environment() {
    let layered = LayeredCredentials::new(
        Box::new(env_creds(&[("FELLOW_EMAIL", "only")])),
        Box::new(MemoryCredentials::with(Credentials::new("key@x", "pw"))),
    );
    assert!(
        layered.load().is_err(),
        "a half-set env must not silently fall through to the keychain"
    );
}

#[test]
fn credentials_debug_never_prints_the_password() {
    assert!(!format!("{:?}", Credentials::new("a", "topsecret")).contains("topsecret"));
}
