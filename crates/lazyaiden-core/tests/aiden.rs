use std::sync::Arc;

use fellow_client::testing::InMemoryFellow;
use fellow_client::{FellowApi, FellowError};
use lazyaiden_core::profiles::{FsProfileStore, PushAction};
use lazyaiden_core::{
    Aiden, Config, ConfigFile, CoreError, Credentials, MemoryCredentials, Overrides, login, logout,
};
use lazyaiden_core::{CredentialStore, DeviceService};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config_in(dir: &std::path::Path) -> Config {
    let o = Overrides {
        config_path: Some(dir.join("config.toml")),
        profiles_dir: Some(dir.join("profiles")),
        ..Overrides::default()
    };
    Config::resolve(&o, &|_| None).unwrap()
}

fn two_brewers() -> Arc<InMemoryFellow> {
    Arc::new(InMemoryFellow::with_brewers(&[
        ("b1", "Kitchen"),
        ("b2", "Office"),
    ]))
}

fn aiden_with(fake: Arc<InMemoryFellow>, dir: &std::path::Path) -> Aiden {
    let config = config_in(dir);
    let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
    Aiden::from_parts(fake, store, config)
}

#[tokio::test]
async fn services_share_one_backend() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(InMemoryFellow::new());
    let aiden = aiden_with(fake.clone(), dir.path());

    let p = aiden
        .profiles
        .create_local(lazyaiden_core::profiles::template("Wired"))
        .unwrap();
    assert_eq!(
        aiden.profiles.push(&p.name).await.unwrap().action,
        PushAction::Created
    );
    assert_eq!(fake.profiles().await.unwrap().len(), 1);

    let s = aiden
        .schedules
        .create(lazyaiden_core::schedules::NewSchedule {
            days: lazyaiden_core::schedules::Days::DAILY,
            time: "6:45".parse().unwrap(),
            water_ml: 400,
            profile: "wired".into(),
            enabled: true,
        })
        .await
        .unwrap();
    assert_eq!(fake.schedules().await.unwrap()[0].id, s.id);
}

#[tokio::test]
async fn configured_brewer_is_selected_at_construction() {
    let dir = tempfile::tempdir().unwrap();
    let fake = two_brewers();
    let mut config = config_in(dir.path());
    config.brewer_id = Some("b2".into());
    let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
    let aiden = Aiden::from_parts(fake.clone(), store, config);

    assert_eq!(aiden.devices.active().await.unwrap().id, "b2");
    assert!(aiden.profiles.list_remote().await.unwrap().is_empty());
}

#[tokio::test]
async fn unselected_multi_brewer_account_asks_for_a_choice() {
    let dir = tempfile::tempdir().unwrap();
    let aiden = aiden_with(two_brewers(), dir.path());
    assert!(matches!(
        aiden.devices.active().await,
        Err(CoreError::Fellow(FellowError::BrewerNotSelected(d))) if d.len() == 2
    ));
}

#[tokio::test]
async fn use_brewer_selects_and_persists_by_id_or_name() {
    let dir = tempfile::tempdir().unwrap();
    let aiden = aiden_with(two_brewers(), dir.path());

    let d = aiden.use_brewer("office").await.unwrap();
    assert_eq!(d.id, "b2");
    assert_eq!(aiden.devices.active().await.unwrap().id, "b2");
    assert_eq!(
        ConfigFile::load(&aiden.config().path)
            .unwrap()
            .brewer_id
            .as_deref(),
        Some("b2")
    );

    aiden.use_brewer("b1").await.unwrap();
    assert_eq!(aiden.config().brewer_id.as_deref(), Some("b1"));

    let before = std::fs::read_to_string(&aiden.config().path).unwrap();
    assert!(matches!(
        aiden.use_brewer("nope").await,
        Err(CoreError::NotFound(_))
    ));
    assert_eq!(
        std::fs::read_to_string(&aiden.config().path).unwrap(),
        before,
        "failed selection must not persist"
    );
}

#[tokio::test]
async fn device_service_listing_and_resolution() {
    let one = DeviceService::new(Arc::new(InMemoryFellow::new()));
    let list = one.list().await.unwrap();
    assert!(
        list.len() == 1 && list[0].active,
        "a single brewer is implicitly active"
    );

    let fake = two_brewers();
    let svc = DeviceService::new(fake.clone());
    assert!(svc.list().await.unwrap().iter().all(|v| !v.active));
    svc.select("Kitchen").await.unwrap();
    let list = svc.list().await.unwrap();
    assert_eq!(
        list.iter()
            .filter(|v| v.active)
            .map(|v| v.device.id.as_str())
            .collect::<Vec<_>>(),
        ["b1"]
    );

    assert_eq!(svc.resolve("b2").await.unwrap().id, "b2");
    assert!(matches!(
        svc.resolve("ghost").await,
        Err(CoreError::NotFound(_))
    ));

    let twins = DeviceService::new(Arc::new(InMemoryFellow::with_brewers(&[
        ("x1", "Same"),
        ("x2", "same"),
    ])));
    assert!(
        matches!(twins.resolve("SAME").await, Err(CoreError::Ambiguous { matches, .. }) if matches.len() == 2)
    );
}

#[tokio::test]
async fn without_credentials_local_features_work_and_remote_ones_explain_login() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());
    let aiden = Aiden::connect(config, &MemoryCredentials::new()).unwrap();

    let p = aiden
        .profiles
        .create_local(lazyaiden_core::profiles::template("Offline"))
        .unwrap();
    assert_eq!(aiden.profiles.list_local().unwrap().profiles.len(), 1);
    assert!(
        aiden
            .profiles
            .export_str(&p.name)
            .unwrap()
            .contains("title: Offline")
    );

    for err in [
        aiden
            .profiles
            .push("offline")
            .await
            .map(|_| ())
            .unwrap_err()
            .to_string(),
        aiden
            .devices
            .list()
            .await
            .map(|_| ())
            .unwrap_err()
            .to_string(),
        aiden
            .schedules
            .list()
            .await
            .map(|_| ())
            .unwrap_err()
            .to_string(),
    ] {
        assert!(err.contains("not logged in"), "{err}");
    }
    assert!(matches!(
        aiden.profiles.status().await,
        Err(lazyaiden_core::profiles::ProfileError::Fellow(
            FellowError::Auth(_)
        ))
    ));
}

#[test]
fn connect_with_credentials_does_no_network_io() {
    let dir = tempfile::tempdir().unwrap();
    let config = config_in(dir.path());

    let creds = MemoryCredentials::with(Credentials::new("a@b.c", "pw"));
    let mut config = config;
    config.base_url = Some("http://127.0.0.1:1".into()); // unreachable: proves construction is offline
    assert!(Aiden::connect(config, &creds).is_ok());
}

#[tokio::test]
async fn login_verifies_before_storing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(json!({"email": "good@x", "password": "pw"})))
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

    let dir = tempfile::tempdir().unwrap();
    let mut config = config_in(dir.path());
    config.base_url = Some(server.uri());
    let store = MemoryCredentials::new();

    let err = login(&config, Credentials::new("bad@x", "nope"), &store)
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Fellow(FellowError::Auth(_))),
        "{err:?}"
    );
    assert!(
        store.load().unwrap().is_none(),
        "bad credentials must not be stored"
    );

    login(&config, Credentials::new("good@x", "pw"), &store)
        .await
        .unwrap();
    assert_eq!(store.load().unwrap().unwrap().email, "good@x");

    logout(&store).unwrap();
    assert!(store.load().unwrap().is_none());
}

struct BrokenStore;
impl CredentialStore for BrokenStore {
    fn load(&self) -> lazyaiden_core::Result<Option<Credentials>> {
        Err(CoreError::Credentials("no keychain".into()))
    }
    fn save(&self, _: &Credentials) -> lazyaiden_core::Result<()> {
        Err(CoreError::Credentials("no keychain".into()))
    }
    fn clear(&self) -> lazyaiden_core::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn an_unreadable_credential_store_degrades_to_logged_out_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let aiden = Aiden::connect(config_in(dir.path()), &BrokenStore).unwrap();
    aiden
        .profiles
        .create_local(lazyaiden_core::profiles::template("Still Works"))
        .unwrap();
    let err = aiden.profiles.status().await.unwrap_err().to_string();
    assert!(
        err.contains("not logged in") && err.contains("no keychain"),
        "{err}"
    );
}
