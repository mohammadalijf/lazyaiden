//! One behavioural suite, run against the in-memory fake directly and against
//! `HttpFellowClient` talking (via wiremock) to a server backed by that fake.
//! This keeps the fake honest and verifies the HTTP routes/bodies end to end.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{draft, fake_server::FakeServer, schedule};
use fellow_client::testing::InMemoryFellow;
use fellow_client::{Credentials, FellowApi, FellowError, HttpClientOptions, HttpFellowClient};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer};

async fn http_over_fake(fake: Arc<InMemoryFellow>) -> (MockServer, HttpFellowClient) {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(FakeServer(fake))
        .mount(&server)
        .await;
    let client = HttpFellowClient::new(
        Credentials::new("a@b.c", "pw"),
        HttpClientOptions {
            base_url: server.uri(),
            retry_base_delay: Duration::ZERO,
            ..HttpClientOptions::default()
        },
    )
    .unwrap();
    (server, client)
}

async fn profile_and_schedule_suite(api: &dyn FellowApi) {
    api.login().await.unwrap();
    assert_eq!(api.active_brewer().await.unwrap(), "b1");
    assert_eq!(api.devices().await.unwrap().len(), 1);
    assert!(api.profiles().await.unwrap().is_empty());
    assert!(api.schedules().await.unwrap().is_empty());

    // create
    let created = api.create_profile(&draft("Morning")).await.unwrap();
    assert!(!created.id.is_empty());
    assert_eq!(created.draft().unwrap(), draft("Morning"));
    assert_eq!(api.profiles().await.unwrap().len(), 1);

    // invalid data is rejected locally and nothing is stored
    let mut bad = draft("Bad");
    bad.ratio = 99.0;
    assert!(matches!(
        api.create_profile(&bad).await,
        Err(FellowError::Validation(_))
    ));
    assert!(matches!(
        api.update_profile(&created.id, &bad).await,
        Err(FellowError::Validation(_))
    ));
    assert_eq!(api.profiles().await.unwrap().len(), 1);

    // update
    let updated = api
        .update_profile(&created.id, &draft("Evening"))
        .await
        .unwrap();
    assert_eq!(updated.id, created.id);
    assert_eq!(api.profiles().await.unwrap()[0].title, "Evening");
    assert!(matches!(
        api.update_profile("p999", &draft("x")).await,
        Err(FellowError::NotFound(_))
    ));

    // share + import
    let link = api.share_profile(&created.id).await.unwrap();
    let shared = api.fetch_shared_profile(&link).await.unwrap();
    assert_eq!(shared.draft().unwrap(), draft("Evening"));
    assert!(matches!(
        api.share_profile("p999").await,
        Err(FellowError::NotFound(_))
    ));
    assert!(matches!(
        api.fetch_shared_profile("nope123").await,
        Err(FellowError::NotFound(_))
    ));

    // schedules
    assert!(matches!(
        api.create_schedule(&schedule("p999")).await,
        Err(FellowError::Api { status: 400, .. })
    ));
    let mut invalid = schedule(&created.id);
    invalid.amount_of_water = 10;
    assert!(matches!(
        api.create_schedule(&invalid).await,
        Err(FellowError::Validation(_))
    ));

    let s = api.create_schedule(&schedule(&created.id)).await.unwrap();
    assert_eq!(s.draft, schedule(&created.id));
    api.toggle_schedule(&s.id, false).await.unwrap();
    assert!(!api.schedules().await.unwrap()[0].draft.enabled);
    assert!(matches!(
        api.toggle_schedule("s999", true).await,
        Err(FellowError::NotFound(_))
    ));
    api.delete_schedule(&s.id).await.unwrap();
    assert!(api.schedules().await.unwrap().is_empty());
    assert!(matches!(
        api.delete_schedule(&s.id).await,
        Err(FellowError::NotFound(_))
    ));

    // settings + delete
    api.adjust_setting("displayName", "Kitchen".into())
        .await
        .unwrap();
    api.delete_profile(&created.id).await.unwrap();
    assert!(api.profiles().await.unwrap().is_empty());
    assert!(matches!(
        api.delete_profile(&created.id).await,
        Err(FellowError::NotFound(_))
    ));
}

async fn multi_brewer_suite(api: &dyn FellowApi) {
    assert_eq!(api.devices().await.unwrap().len(), 2);
    assert!(matches!(api.profiles().await, Err(FellowError::BrewerNotSelected(d)) if d.len() == 2));
    assert!(matches!(
        api.active_brewer().await,
        Err(FellowError::BrewerNotSelected(_))
    ));

    api.select_brewer("b2");
    assert_eq!(api.selected_brewer().as_deref(), Some("b2"));
    assert_eq!(api.active_brewer().await.unwrap(), "b2");
    api.create_profile(&draft("Only on b2")).await.unwrap();
    assert_eq!(api.profiles().await.unwrap().len(), 1);

    api.select_brewer("b1");
    assert!(
        api.profiles().await.unwrap().is_empty(),
        "profiles are per brewer"
    );

    api.select_brewer("ghost");
    assert!(matches!(
        api.profiles().await,
        Err(FellowError::NotFound(_))
    ));
}

#[tokio::test]
async fn fake_passes_profile_and_schedule_suite() {
    profile_and_schedule_suite(&InMemoryFellow::new()).await;
}

#[tokio::test]
async fn http_passes_profile_and_schedule_suite() {
    let (_server, client) = http_over_fake(Arc::new(InMemoryFellow::new())).await;
    profile_and_schedule_suite(&client).await;
}

#[tokio::test]
async fn fake_passes_multi_brewer_suite() {
    multi_brewer_suite(&InMemoryFellow::with_brewers(&[
        ("b1", "One"),
        ("b2", "Two"),
    ]))
    .await;
}

#[tokio::test]
async fn http_passes_multi_brewer_suite() {
    let fake = Arc::new(InMemoryFellow::with_brewers(&[
        ("b1", "One"),
        ("b2", "Two"),
    ]));
    let (_server, client) = http_over_fake(fake).await;
    multi_brewer_suite(&client).await;
}

#[tokio::test]
async fn fake_with_no_brewers_reports_no_devices() {
    let api = InMemoryFellow::with_brewers(&[]);
    assert!(matches!(api.profiles().await, Err(FellowError::NoDevices)));
}

#[tokio::test]
async fn fake_seeded_share_drops_server_fields_in_draft() {
    let api = InMemoryFellow::new();
    api.seed_shared("abc123", &draft("Shared"));
    let p = api
        .fetch_shared_profile("https://fellowproducts.com/p/abc123")
        .await
        .unwrap();
    assert!(p.extra.contains_key("sharedFrom"));
    assert_eq!(p.draft().unwrap(), draft("Shared"));
    let created = api.create_profile(&p.draft().unwrap()).await.unwrap();
    assert!(!created.extra.contains_key("sharedFrom"));
}
