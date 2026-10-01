use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{FellowApi, brew_id_from_link};
use crate::error::{FellowError, Result};
use crate::models::{Device, Profile, ProfileDraft, Schedule, ScheduleDraft};

/// Production API endpoint.
pub const DEFAULT_BASE_URL: &str = "https://l8qtmnc692.execute-api.us-west-2.amazonaws.com/v1";
/// The backend only serves the official app's user agent.
pub const DEFAULT_USER_AGENT: &str = "Fellow/5 CFNetwork/1568.300.101 Darwin/24.2.0";

const RETRYABLE: &[u16] = &[408, 500, 502, 503, 504];

/// Fellow account credentials.
#[derive(Clone)]
pub struct Credentials {
    /// Account email.
    pub email: String,
    /// Account password.
    pub password: String,
}

impl Credentials {
    /// Creates credentials.
    pub fn new(email: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            password: password.into(),
        }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("email", &self.email)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Tunables for [`HttpFellowClient`].
#[derive(Debug, Clone)]
pub struct HttpClientOptions {
    /// API root without trailing slash.
    pub base_url: String,
    /// `User-Agent` header value.
    pub user_agent: String,
    /// Retries for transient failures (network errors and 408/5xx).
    pub max_retries: u32,
    /// Base back-off; attempt *n* waits `n × retry_base_delay`.
    pub retry_base_delay: Duration,
    /// Per-request timeout.
    pub timeout: Duration,
}

impl Default for HttpClientOptions {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            user_agent: DEFAULT_USER_AGENT.into(),
            max_retries: 3,
            retry_base_delay: Duration::from_millis(300),
            timeout: Duration::from_secs(30),
        }
    }
}

/// Default [`FellowApi`] implementation over HTTPS.
///
/// Logs in lazily, re-authenticates once on `401`, and retries transient
/// failures with linear back-off.
pub struct HttpFellowClient {
    http: reqwest::Client,
    credentials: Credentials,
    options: HttpClientOptions,
    token: Mutex<Option<String>>,
    brewer: Mutex<Option<String>>,
}

impl fmt::Debug for HttpFellowClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpFellowClient")
            .field("base_url", &self.options.base_url)
            .field("credentials", &self.credentials)
            .finish_non_exhaustive()
    }
}

/// Percent-encodes a path segment.
fn seg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode<T: DeserializeOwned>(text: &str, what: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|e| FellowError::Decode(format!("{what}: {e}")))
}

fn status_error(status: u16, body: String) -> FellowError {
    match status {
        401 | 403 => FellowError::Auth(body),
        404 => FellowError::NotFound(body),
        _ => FellowError::Api { status, body },
    }
}

impl HttpFellowClient {
    /// Creates a client. No network traffic happens until the first call.
    pub fn new(credentials: Credentials, mut options: HttpClientOptions) -> Result<Self> {
        options.base_url = options.base_url.trim_end_matches('/').to_owned();
        let http = reqwest::Client::builder()
            .user_agent(&options.user_agent)
            .timeout(options.timeout)
            .build()
            .map_err(|e| FellowError::Network(e.to_string()))?;
        Ok(Self {
            http,
            credentials,
            options,
            token: Mutex::new(None),
            brewer: Mutex::new(None),
        })
    }

    fn token(&self) -> Option<String> {
        self.token.lock().expect("token lock").clone()
    }

    /// One logical request: transient failures are retried, no auth handling.
    async fn execute(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        token: Option<&str>,
    ) -> Result<(u16, String)> {
        let url = format!("{}{path}", self.options.base_url);
        let mut attempt = 0u32;
        loop {
            let mut req = self
                .http
                .request(method.clone(), &url)
                .header("Accept", "application/json");
            if let Some(t) = token {
                req = req.bearer_auth(t);
            }
            if let Some(b) = body {
                req = req.json(b);
            }
            let retry_wait = self.options.retry_base_delay * (attempt + 1);
            match req.send().await {
                Err(e) => {
                    if attempt >= self.options.max_retries {
                        return Err(FellowError::Network(e.to_string()));
                    }
                }
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if RETRYABLE.contains(&status) && attempt < self.options.max_retries {
                        // fall through to the back-off below
                    } else {
                        let text = resp
                            .text()
                            .await
                            .map_err(|e| FellowError::Network(e.to_string()))?;
                        return Ok((status, text));
                    }
                }
            }
            attempt += 1;
            tokio::time::sleep(retry_wait).await;
        }
    }

    async fn authenticate(&self) -> Result<()> {
        let body = json!({
            "email": self.credentials.email,
            "password": self.credentials.password,
        });
        let (status, text) = self
            .execute(Method::POST, "/auth/login", Some(&body), None)
            .await?;
        if !(200..300).contains(&status) {
            return Err(if (400..500).contains(&status) {
                FellowError::Auth("email or password incorrect".into())
            } else {
                FellowError::Api { status, body: text }
            });
        }
        let parsed: Value = decode(&text, "login response")?;
        let token = parsed
            .get("accessToken")
            .and_then(Value::as_str)
            .ok_or_else(|| FellowError::Auth("login response had no accessToken".into()))?;
        *self.token.lock().expect("token lock") = Some(token.to_owned());
        Ok(())
    }

    /// Authenticated request: logs in lazily and re-authenticates once on 401.
    async fn request(&self, method: Method, path: &str, body: Option<&Value>) -> Result<String> {
        if self.token().is_none() {
            self.authenticate().await?;
        }
        let mut reauthed = false;
        loop {
            let token = self.token();
            let (status, text) = self
                .execute(method.clone(), path, body, token.as_deref())
                .await?;
            if status == 401 && !reauthed {
                reauthed = true;
                self.authenticate().await?;
                continue;
            }
            return if (200..300).contains(&status) {
                Ok(text)
            } else {
                Err(status_error(status, text))
            };
        }
    }

    async fn brewer_id(&self) -> Result<String> {
        if let Some(id) = self.selected_brewer() {
            return Ok(id);
        }
        let mut devices = self.devices().await?;
        match devices.len() {
            0 => Err(FellowError::NoDevices),
            1 => {
                let id = devices.remove(0).id;
                *self.brewer.lock().expect("brewer lock") = Some(id.clone());
                Ok(id)
            }
            _ => Err(FellowError::BrewerNotSelected(devices)),
        }
    }

    async fn device_path(&self, tail: &str) -> Result<String> {
        Ok(format!("/devices/{}{tail}", seg(&self.brewer_id().await?)))
    }
}

#[async_trait]
impl FellowApi for HttpFellowClient {
    async fn login(&self) -> Result<()> {
        self.authenticate().await
    }

    async fn devices(&self) -> Result<Vec<Device>> {
        let text = self
            .request(Method::GET, "/devices?dataType=real", None)
            .await?;
        decode(&text, "device list")
    }

    fn select_brewer(&self, id: &str) {
        *self.brewer.lock().expect("brewer lock") = Some(id.to_owned());
    }

    fn selected_brewer(&self) -> Option<String> {
        self.brewer.lock().expect("brewer lock").clone()
    }

    async fn active_brewer(&self) -> Result<String> {
        self.brewer_id().await
    }

    async fn adjust_setting(&self, setting: &str, value: Value) -> Result<Value> {
        let path = self.device_path("").await?;
        let body = json!({ setting: value });
        let text = self.request(Method::PATCH, &path, Some(&body)).await?;
        Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
    }

    async fn profiles(&self) -> Result<Vec<Profile>> {
        let path = self.device_path("/profiles").await?;
        decode(
            &self.request(Method::GET, &path, None).await?,
            "profile list",
        )
    }

    async fn create_profile(&self, draft: &ProfileDraft) -> Result<Profile> {
        draft.validate()?;
        let path = self.device_path("/profiles").await?;
        let body = serde_json::to_value(draft).map_err(|e| FellowError::Decode(e.to_string()))?;
        let text = self.request(Method::POST, &path, Some(&body)).await?;
        profile_response(&text)
    }

    async fn update_profile(&self, profile_id: &str, draft: &ProfileDraft) -> Result<Profile> {
        draft.validate()?;
        let path = self
            .device_path(&format!("/profiles/{}", seg(profile_id)))
            .await?;
        let body = serde_json::to_value(draft).map_err(|e| FellowError::Decode(e.to_string()))?;
        let text = self.request(Method::PATCH, &path, Some(&body)).await?;
        profile_response(&text)
    }

    async fn delete_profile(&self, profile_id: &str) -> Result<()> {
        let path = self
            .device_path(&format!("/profiles/{}", seg(profile_id)))
            .await?;
        self.request(Method::DELETE, &path, None).await.map(|_| ())
    }

    async fn share_profile(&self, profile_id: &str) -> Result<String> {
        let path = self
            .device_path(&format!("/profiles/{}/share", seg(profile_id)))
            .await?;
        let text = self.request(Method::POST, &path, None).await?;
        let parsed: Value = decode(&text, "share response")?;
        parsed
            .get("link")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| FellowError::Decode(format!("share response had no link: {text}")))
    }

    async fn fetch_shared_profile(&self, link_or_id: &str) -> Result<Profile> {
        let id = brew_id_from_link(link_or_id)?;
        let text = self
            .request(Method::GET, &format!("/shared/{}", seg(&id)), None)
            .await?;
        decode(&text, "shared profile")
    }

    async fn schedules(&self) -> Result<Vec<Schedule>> {
        let path = self.device_path("/schedules").await?;
        decode(
            &self.request(Method::GET, &path, None).await?,
            "schedule list",
        )
    }

    async fn create_schedule(&self, draft: &ScheduleDraft) -> Result<Schedule> {
        draft.validate()?;
        let path = self.device_path("/schedules").await?;
        let body = serde_json::to_value(draft).map_err(|e| FellowError::Decode(e.to_string()))?;
        let text = self.request(Method::POST, &path, Some(&body)).await?;
        let schedule: Schedule = decode(&text, "created schedule")?;
        Ok(schedule)
    }

    async fn toggle_schedule(&self, schedule_id: &str, enabled: bool) -> Result<()> {
        let path = self
            .device_path(&format!("/schedules/{}", seg(schedule_id)))
            .await?;
        let body = json!({ "enabled": enabled });
        self.request(Method::PATCH, &path, Some(&body))
            .await
            .map(|_| ())
    }

    async fn delete_schedule(&self, schedule_id: &str) -> Result<()> {
        let path = self
            .device_path(&format!("/schedules/{}", seg(schedule_id)))
            .await?;
        self.request(Method::DELETE, &path, None).await.map(|_| ())
    }
}

fn profile_response(text: &str) -> Result<Profile> {
    let profile: Profile = decode(text, "profile response")?;
    if profile.id.is_empty() {
        return Err(FellowError::Decode(format!(
            "profile response had no id: {text}"
        )));
    }
    Ok(profile)
}
