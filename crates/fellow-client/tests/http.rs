mod common;

use std::time::Duration;

use common::{draft, schedule};
use fellow_client::{Credentials, FellowApi, FellowError, HttpClientOptions, HttpFellowClient};
use serde_json::{Value, json};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer) -> HttpFellowClient {
    HttpFellowClient::new(
        Credentials::new("me@example.com", "hunter2"),
        HttpClientOptions {
            base_url: server.uri(),
            retry_base_delay: Duration::ZERO,
            ..HttpClientOptions::default()
        },
    )
    .unwrap()
}

fn login_ok(token: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({"accessToken": token, "refreshToken": "r"}))
}

async fn one_device(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/devices"))
        .and(query_param("dataType", "real"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"id": "b1", "displayName": "Aiden"}])),
        )
        .mount(server)
        .await;
}

async fn requests_to(server: &MockServer, verb: &str, p: &str) -> usize {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == verb && r.url.path() == p)
        .count()
}

#[test]
fn credentials_debug_redacts_password() {
    let dbg = format!("{:?}", Credentials::new("a@b.c", "hunter2"));
    assert!(dbg.contains("a@b.c") && !dbg.contains("hunter2"));
}

#[tokio::test]
async fn login_posts_credentials_with_the_app_user_agent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .and(body_json(
            json!({"email": "me@example.com", "password": "hunter2"}),
        ))
        .and(header(
            "user-agent",
            fellow_client::HttpClientOptions::default()
                .user_agent
                .as_str(),
        ))
        .respond_with(login_ok("tok"))
        .expect(1)
        .mount(&server)
        .await;
    client_for(&server).login().await.unwrap();
}

#[tokio::test]
async fn bad_credentials_give_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;
    assert!(matches!(
        client_for(&server).login().await,
        Err(FellowError::Auth(_))
    ));
}

#[tokio::test]
async fn login_response_without_token_is_an_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message": "hi"})))
        .mount(&server)
        .await;
    assert!(matches!(
        client_for(&server).login().await,
        Err(FellowError::Auth(_))
    ));
}

#[tokio::test]
async fn logs_in_lazily_once_and_sends_bearer_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("tok1"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .and(header("authorization", "Bearer tok1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id": "b1"}])))
        .expect(2)
        .mount(&server)
        .await;
    let c = client_for(&server);
    c.devices().await.unwrap();
    c.devices().await.unwrap();
}

#[tokio::test]
async fn reauthenticates_once_on_401_and_retries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("old"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("new"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("GET"))
        .and(path("/devices/b1/profiles"))
        .and(header("authorization", "Bearer old"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices/b1/profiles"))
        .and(header("authorization", "Bearer new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id": "p1", "title": "A"}])))
        .mount(&server)
        .await;
    // `devices()` is called with the old token too; it succeeds for either token.
    let profiles = client_for(&server).profiles().await.unwrap();
    assert_eq!(profiles[0].id, "p1");
    assert_eq!(requests_to(&server, "POST", "/auth/login").await, 2);
}

#[tokio::test]
async fn persistent_401_does_not_loop() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("GET"))
        .and(path("/devices/b1/profiles"))
        .respond_with(ResponseTemplate::new(401).set_body_string("denied"))
        .mount(&server)
        .await;
    let err = client_for(&server).profiles().await.unwrap_err();
    assert!(matches!(err, FellowError::Auth(_)), "{err:?}");
    assert_eq!(requests_to(&server, "GET", "/devices/b1/profiles").await, 2);
    assert_eq!(requests_to(&server, "POST", "/auth/login").await, 2);
}

#[tokio::test]
async fn retries_transient_503_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id": "b1"}])))
        .mount(&server)
        .await;
    assert_eq!(client_for(&server).devices().await.unwrap().len(), 1);
    assert_eq!(requests_to(&server, "GET", "/devices").await, 3);
}

#[tokio::test]
async fn gives_up_after_max_retries_with_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(ResponseTemplate::new(503).set_body_string("down"))
        .mount(&server)
        .await;
    let err = client_for(&server).devices().await.unwrap_err();
    assert!(
        matches!(err, FellowError::Api { status: 503, .. }),
        "{err:?}"
    );
    assert_eq!(requests_to(&server, "GET", "/devices").await, 4); // 1 + 3 retries
}

#[tokio::test]
async fn non_retryable_client_errors_are_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad"))
        .mount(&server)
        .await;
    assert!(matches!(
        client_for(&server).devices().await,
        Err(FellowError::Api { status: 400, .. })
    ));
    assert_eq!(requests_to(&server, "GET", "/devices").await, 1);
}

#[tokio::test]
async fn unreachable_server_gives_network_error() {
    let c = HttpFellowClient::new(
        Credentials::new("a", "b"),
        HttpClientOptions {
            base_url: "http://127.0.0.1:1".into(),
            max_retries: 1,
            retry_base_delay: Duration::ZERO,
            ..HttpClientOptions::default()
        },
    )
    .unwrap();
    assert!(matches!(c.login().await, Err(FellowError::Network(_))));
}

#[tokio::test]
async fn create_profile_sends_only_editable_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    let d = draft("Sent");
    Mock::given(method("POST"))
        .and(path("/devices/b1/profiles"))
        .and(body_json(serde_json::to_value(&d).unwrap()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "p5", "title": "Sent"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        client_for(&server).create_profile(&d).await.unwrap().id,
        "p5"
    );
}

#[tokio::test]
async fn invalid_profile_is_rejected_before_any_request() {
    let server = MockServer::start().await;
    let mut d = draft("x");
    d.bloom_duration = 500;
    let err = client_for(&server).create_profile(&d).await.unwrap_err();
    assert!(matches!(err, FellowError::Validation(ref v) if v[0].field == "bloomDuration"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn create_response_without_id_is_a_decode_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("POST"))
        .and(path("/devices/b1/profiles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message": "weird"})))
        .mount(&server)
        .await;
    assert!(matches!(
        client_for(&server).create_profile(&draft("x")).await,
        Err(FellowError::Decode(_))
    ));
}

#[tokio::test]
async fn toggle_schedule_patches_enabled_flag() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("PATCH"))
        .and(path("/devices/b1/schedules/s1"))
        .and(body_json(json!({"enabled": false})))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client_for(&server)
        .toggle_schedule("s1", false)
        .await
        .unwrap();
}

#[tokio::test]
async fn create_schedule_sends_api_shape() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    let body = json!({"days":[false,true,true,true,true,true,false],"secondFromStartOfTheDay":27000,"enabled":true,"amountOfWater":500,"profileId":"p1"});
    let mut reply = body.clone();
    reply["id"] = Value::from("s1");
    Mock::given(method("POST"))
        .and(path("/devices/b1/schedules"))
        .and(body_json(body))
        .respond_with(ResponseTemplate::new(200).set_body_json(reply))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        client_for(&server)
            .create_schedule(&schedule("p1"))
            .await
            .unwrap()
            .id,
        "s1"
    );
}

#[tokio::test]
async fn missing_resource_is_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(404).set_body_string("gone"))
        .mount(&server)
        .await;
    assert!(matches!(
        client_for(&server).delete_profile("p1").await,
        Err(FellowError::NotFound(_))
    ));
}

#[tokio::test]
async fn brewer_selection_rules() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"id": "b1"}, {"id": "b2", "displayName": "Two"}])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices/b2/profiles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let c = client_for(&server);
    assert!(matches!(c.profiles().await, Err(FellowError::BrewerNotSelected(d)) if d.len() == 2));
    assert_eq!(c.selected_brewer(), None);
    c.select_brewer("b2");
    assert!(c.profiles().await.unwrap().is_empty());
}

#[tokio::test]
async fn single_brewer_is_selected_automatically_and_empty_account_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("GET"))
        .and(path("/devices/b1/schedules"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    let c = client_for(&server);
    c.schedules().await.unwrap();
    assert_eq!(c.selected_brewer().as_deref(), Some("b1"));

    let empty = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&empty)
        .await;
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&empty)
        .await;
    assert!(matches!(
        client_for(&empty).profiles().await,
        Err(FellowError::NoDevices)
    ));
}

#[tokio::test]
async fn ids_are_percent_encoded_in_paths() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("DELETE"))
        .and(path("/devices/b1/profiles/a%2Fb"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    client_for(&server).delete_profile("a/b").await.unwrap();
}

#[tokio::test]
async fn share_and_import_by_link() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("POST"))
        .and(path("/devices/b1/profiles/p1/share"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"link": "https://fellowproducts.com/p/xyz9"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/shared/xyz9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "orig", "title": "T"})))
        .expect(1)
        .mount(&server)
        .await;
    let c = client_for(&server);
    let link = c.share_profile("p1").await.unwrap();
    assert_eq!(c.fetch_shared_profile(&link).await.unwrap().title, "T");
}

#[tokio::test]
async fn share_response_without_link_is_decode_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/login"))
        .respond_with(login_ok("t"))
        .mount(&server)
        .await;
    one_device(&server).await;
    Mock::given(method("POST"))
        .and(path("/devices/b1/profiles/p1/share"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    assert!(matches!(
        client_for(&server).share_profile("p1").await,
        Err(FellowError::Decode(_))
    ));
}
