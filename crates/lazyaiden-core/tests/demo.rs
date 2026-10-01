#![cfg(feature = "demo")]

use lazyaiden_core::Aiden;

#[tokio::test]
async fn demo_backend_is_seeded_and_usable_offline() {
    let aiden = Aiden::demo(None).await.unwrap();

    assert_eq!(aiden.devices.active().await.unwrap().id, "demo-brewer");
    assert_eq!(aiden.profiles.list_remote().await.unwrap().len(), 2);
    assert_eq!(aiden.schedules.list().await.unwrap().len(), 1);
    assert_eq!(aiden.profiles.list_local().unwrap().profiles.len(), 1);
    let pulled = aiden.profiles.pull_all().await.unwrap();
    assert!(pulled.iter().all(|(_, r)| r.is_ok()));
    assert_eq!(aiden.profiles.list_local().unwrap().profiles.len(), 3);
}

#[tokio::test]
async fn demo_never_touches_the_configured_directories() {
    let aiden = Aiden::demo(None).await.unwrap();
    let config = aiden.config();
    let root = config.path.parent().unwrap().to_owned();
    assert!(config.profiles_dir.starts_with(&root));
    let name = root.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("lazyaiden-demo-"), "{}", root.display());

    aiden.use_brewer("demo-brewer").await.unwrap();
    assert!(
        config.path.is_file(),
        "the brewer choice lands in the demo config"
    );
    drop(aiden);
    assert!(!root.exists(), "the demo directory is removed on drop");
}

#[tokio::test]
async fn an_explicit_profiles_dir_is_shared_between_demo_runs_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let profiles = dir.path().join("profiles");

    let first = Aiden::demo(Some(profiles.clone())).await.unwrap();
    first
        .profiles
        .create_local(lazyaiden_core::profiles::template("Kept"))
        .unwrap();
    drop(first);

    let second = Aiden::demo(Some(profiles.clone())).await.unwrap();
    let titles: Vec<String> = second
        .profiles
        .list_local()
        .unwrap()
        .profiles
        .into_iter()
        .map(|p| p.draft.title)
        .collect();
    assert_eq!(titles.len(), 2, "{titles:?}");
    assert!(titles.contains(&"Kept".to_owned()), "{titles:?}");
    let outcome = second.profiles.push("Kept").await.unwrap();
    assert_eq!(
        outcome.action,
        lazyaiden_core::profiles::PushAction::Created
    );
    drop(second);
    assert!(profiles.is_dir(), "an explicit directory outlives the demo");
}
