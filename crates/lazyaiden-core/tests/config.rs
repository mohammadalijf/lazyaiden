use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use lazyaiden_core::{Config, ConfigFile, CoreError, Overrides};

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    move |k| map.get(k).cloned().filter(|v| !v.is_empty())
}

#[test]
fn defaults_derive_from_home() {
    let c = Config::resolve(&Overrides::default(), &env_of(&[("HOME", "/h")])).unwrap();
    assert_eq!(c.path, PathBuf::from("/h/.config/lazyaiden/config.toml"));
    assert_eq!(
        c.profiles_dir,
        PathBuf::from("/h/.config/lazyaiden/profiles")
    );
    assert_eq!((c.base_url, c.brewer_id), (None, None));
}

#[test]
fn xdg_config_home_wins_over_home() {
    let c = Config::resolve(
        &Overrides::default(),
        &env_of(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "/x")]),
    )
    .unwrap();
    assert_eq!(c.path, PathBuf::from("/x/lazyaiden/config.toml"));
}

#[test]
fn no_home_is_a_config_error() {
    assert!(matches!(
        Config::resolve(&Overrides::default(), &env_of(&[])),
        Err(CoreError::Config { .. })
    ));
}

#[test]
fn precedence_is_flag_then_env_then_file_then_default() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(
        &cfg,
        "profiles_dir = \"/file/profiles\"\nbrewer_id = \"file-b\"\nbase_url = \"http://file\"\n",
    )
    .unwrap();
    let base = Overrides {
        config_path: Some(cfg.clone()),
        ..Overrides::default()
    };

    // file only
    let c = Config::resolve(&base, &env_of(&[])).unwrap();
    assert_eq!(c.profiles_dir, PathBuf::from("/file/profiles"));
    assert_eq!(c.brewer_id.as_deref(), Some("file-b"));
    assert_eq!(c.base_url.as_deref(), Some("http://file"));

    // env beats file
    let env = env_of(&[
        ("LAZYAIDEN_PROFILES_DIR", "/env/p"),
        ("LAZYAIDEN_BREWER_ID", "env-b"),
        ("LAZYAIDEN_BASE_URL", "http://env"),
    ]);
    let c = Config::resolve(&base, &env).unwrap();
    assert_eq!(c.profiles_dir, PathBuf::from("/env/p"));
    assert_eq!(c.brewer_id.as_deref(), Some("env-b"));
    assert_eq!(c.base_url.as_deref(), Some("http://env"));

    // flags beat env
    let flags = Overrides {
        profiles_dir: Some("/flag/p".into()),
        brewer_id: Some("flag-b".into()),
        base_url: Some("http://flag".into()),
        ..base
    };
    let c = Config::resolve(&flags, &env).unwrap();
    assert_eq!(c.profiles_dir, PathBuf::from("/flag/p"));
    assert_eq!(c.brewer_id.as_deref(), Some("flag-b"));
    assert_eq!(c.base_url.as_deref(), Some("http://flag"));
}

#[test]
fn empty_env_values_are_ignored() {
    let c = Config::resolve(
        &Overrides::default(),
        &env_of(&[("HOME", "/h"), ("LAZYAIDEN_BREWER_ID", "")]),
    )
    .unwrap();
    assert_eq!(c.brewer_id, None);
}

#[test]
fn aiden_config_env_selects_the_file_and_default_profiles_dir_sits_beside_it() {
    let c = Config::resolve(
        &Overrides::default(),
        &env_of(&[("LAZYAIDEN_CONFIG", "/custom/dir/my.toml")]),
    )
    .unwrap();
    assert_eq!(c.path, PathBuf::from("/custom/dir/my.toml"));
    assert_eq!(c.profiles_dir, PathBuf::from("/custom/dir/profiles"));
}

#[test]
fn tilde_is_expanded() {
    let o = Overrides {
        profiles_dir: Some("~/brews".into()),
        ..Overrides::default()
    };
    let c = Config::resolve(&o, &env_of(&[("HOME", "/h")])).unwrap();
    assert_eq!(c.profiles_dir, PathBuf::from("/h/brews"));
}

#[test]
fn invalid_toml_and_unknown_keys_are_reported_with_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    let o = Overrides {
        config_path: Some(cfg.clone()),
        ..Overrides::default()
    };

    fs::write(&cfg, "profiles_dir = [").unwrap();
    assert!(
        matches!(Config::resolve(&o, &env_of(&[])), Err(CoreError::Config { path, .. }) if path == cfg)
    );

    fs::write(&cfg, "profile_dir = \"/typo\"\n").unwrap();
    let err = Config::resolve(&o, &env_of(&[])).unwrap_err().to_string();
    assert!(err.contains("profile_dir"), "{err}");
}

#[test]
fn persist_brewer_keeps_other_keys_and_creates_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("nested/config.toml");
    let o = Overrides {
        config_path: Some(cfg.clone()),
        ..Overrides::default()
    };

    let mut c = Config::resolve(&o, &env_of(&[])).unwrap();
    c.persist_brewer("b7").unwrap();
    assert_eq!(c.brewer_id.as_deref(), Some("b7"));
    assert_eq!(
        ConfigFile::load(&cfg).unwrap().brewer_id.as_deref(),
        Some("b7")
    );

    fs::write(&cfg, "profiles_dir = \"/keep\"\nbrewer_id = \"old\"\n").unwrap();
    let mut c = Config::resolve(&o, &env_of(&[])).unwrap();
    c.persist_brewer("new").unwrap();
    let file = ConfigFile::load(&cfg).unwrap();
    assert_eq!(file.profiles_dir, Some(PathBuf::from("/keep")));
    assert_eq!(file.brewer_id.as_deref(), Some("new"));
}

#[test]
fn persisting_does_not_bake_env_overrides_into_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    let o = Overrides {
        config_path: Some(cfg.clone()),
        ..Overrides::default()
    };
    let mut c = Config::resolve(&o, &env_of(&[("LAZYAIDEN_PROFILES_DIR", "/env/only")])).unwrap();
    c.persist_brewer("b1").unwrap();
    assert_eq!(ConfigFile::load(&cfg).unwrap().profiles_dir, None);
}
