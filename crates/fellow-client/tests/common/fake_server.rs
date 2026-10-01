//! A wiremock responder that serves the Fellow HTTP API from an `InMemoryFellow`.
//! Lets the contract suite run through the real `HttpFellowClient`.
#![allow(dead_code)]

use std::sync::Arc;

use fellow_client::testing::InMemoryFellow;
use fellow_client::{FellowApi, FellowError, ProfileDraft, ScheduleDraft};
use serde_json::{Value, json};
use wiremock::{Request, Respond, ResponseTemplate};

pub struct FakeServer(pub Arc<InMemoryFellow>);

impl Respond for FakeServer {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let fake = self.0.clone();
        let method = req.method.to_string();
        let path = req.url.path().to_owned();
        let body: Option<Value> = serde_json::from_slice(&req.body).ok();
        ready(route(fake, method, path, body))
    }
}

/// `InMemoryFellow` futures never suspend, so a single poll completes them.
/// (wiremock responders are sync and run on a separate runtime.)
fn ready<T>(fut: impl std::future::Future<Output = T>) -> T {
    let mut fut = std::pin::pin!(fut);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match fut.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(v) => v,
        std::task::Poll::Pending => unreachable!("in-memory fake must not suspend"),
    }
}

fn ok_json<T: serde::Serialize>(v: &T) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(v)
}

fn err(e: FellowError) -> ResponseTemplate {
    match e {
        FellowError::NotFound(m) => ResponseTemplate::new(404).set_body_string(m),
        FellowError::Api { status, body } => ResponseTemplate::new(status).set_body_string(body),
        other => ResponseTemplate::new(400).set_body_string(other.to_string()),
    }
}

fn done<T: serde::Serialize>(r: Result<T, FellowError>) -> ResponseTemplate {
    r.map_or_else(err, |v| ok_json(&v))
}

async fn route(
    fake: Arc<InMemoryFellow>,
    method: String,
    path: String,
    body: Option<Value>,
) -> ResponseTemplate {
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let body = body.unwrap_or(Value::Null);
    match (method.as_str(), segs.as_slice()) {
        ("POST", ["auth", "login"]) => {
            ok_json(&json!({"accessToken": "tok", "refreshToken": "ref"}))
        }
        ("GET", ["devices"]) => done(fake.devices().await),
        ("GET", ["shared", id]) => done(fake.fetch_shared_profile(id).await),
        (m, ["devices", id, rest @ ..]) => {
            fake.select_brewer(id);
            match (m, rest) {
                ("PATCH", []) => {
                    let (k, v) = body
                        .as_object()
                        .and_then(|o| o.iter().next())
                        .expect("one setting");
                    done(fake.adjust_setting(k, v.clone()).await)
                }
                ("GET", ["profiles"]) => done(fake.profiles().await),
                ("POST", ["profiles"]) => match serde_json::from_value::<ProfileDraft>(body) {
                    Ok(d) => done(fake.create_profile(&d).await),
                    Err(e) => ResponseTemplate::new(400).set_body_string(e.to_string()),
                },
                ("PATCH", ["profiles", pid]) => {
                    match serde_json::from_value::<ProfileDraft>(body) {
                        Ok(d) => done(fake.update_profile(pid, &d).await),
                        Err(e) => ResponseTemplate::new(400).set_body_string(e.to_string()),
                    }
                }
                ("DELETE", ["profiles", pid]) => fake
                    .delete_profile(pid)
                    .await
                    .map_or_else(err, |()| ResponseTemplate::new(200)),
                ("POST", ["profiles", pid, "share"]) => {
                    done(fake.share_profile(pid).await.map(|l| json!({"link": l})))
                }
                ("GET", ["schedules"]) => done(fake.schedules().await),
                ("POST", ["schedules"]) => match serde_json::from_value::<ScheduleDraft>(body) {
                    Ok(d) => done(fake.create_schedule(&d).await),
                    Err(e) => ResponseTemplate::new(400).set_body_string(e.to_string()),
                },
                ("PATCH", ["schedules", sid]) => {
                    let enabled = body["enabled"].as_bool().expect("enabled");
                    fake.toggle_schedule(sid, enabled)
                        .await
                        .map_or_else(err, |()| ResponseTemplate::new(200))
                }
                ("DELETE", ["schedules", sid]) => fake
                    .delete_schedule(sid)
                    .await
                    .map_or_else(err, |()| ResponseTemplate::new(200)),
                _ => ResponseTemplate::new(404).set_body_string("no such route"),
            }
        }
        _ => ResponseTemplate::new(404).set_body_string("no such route"),
    }
}
