use std::sync::Mutex;

pub use fellow_client::Credentials;
use serde::{Deserialize, Serialize};

use crate::config::Env;
use crate::error::{CoreError, Result};

/// Where Fellow account credentials come from and go to.
pub trait CredentialStore: Send + Sync {
    /// The stored credentials, if any.
    fn load(&self) -> Result<Option<Credentials>>;
    /// Stores credentials.
    fn save(&self, credentials: &Credentials) -> Result<()>;
    /// Forgets stored credentials (a no-op when none exist).
    fn clear(&self) -> Result<()>;
}

fn store_error(e: impl ToString) -> CoreError {
    CoreError::Credentials(e.to_string())
}

/// Read-only credentials from `FELLOW_EMAIL` and `FELLOW_PASSWORD`.
pub struct EnvCredentials<F> {
    env: F,
}

impl EnvCredentials<fn(&str) -> Option<String>> {
    /// Reads the real process environment.
    pub fn from_process() -> Self {
        Self {
            env: crate::config::process_env,
        }
    }
}

impl<F: Fn(&str) -> Option<String> + Send + Sync> EnvCredentials<F> {
    /// Reads from a custom environment (for tests).
    pub fn new(env: F) -> Self {
        Self { env }
    }
}

impl<F: Fn(&str) -> Option<String> + Send + Sync> CredentialStore for EnvCredentials<F> {
    fn load(&self) -> Result<Option<Credentials>> {
        let env: Env = &self.env;
        match (env("FELLOW_EMAIL"), env("FELLOW_PASSWORD")) {
            (Some(email), Some(password)) => Ok(Some(Credentials::new(email, password))),
            (None, None) => Ok(None),
            (Some(_), None) => Err(store_error(
                "FELLOW_EMAIL is set but FELLOW_PASSWORD is not",
            )),
            (None, Some(_)) => Err(store_error(
                "FELLOW_PASSWORD is set but FELLOW_EMAIL is not",
            )),
        }
    }

    fn save(&self, _: &Credentials) -> Result<()> {
        Err(store_error("environment variables are read-only"))
    }

    fn clear(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Secret {
    email: String,
    password: String,
}

/// The OS keychain (macOS Keychain, Windows Credential Manager, Secret Service).
///
/// The email and password are stored together as one entry.
pub struct KeyringCredentials {
    service: String,
    user: String,
}

impl Default for KeyringCredentials {
    fn default() -> Self {
        Self {
            service: "lazyaiden".into(),
            user: "fellow-account".into(),
        }
    }
}

impl KeyringCredentials {
    /// Uses the default entry (`lazyaiden` / `fellow-account`).
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.user).map_err(store_error)
    }
}

impl CredentialStore for KeyringCredentials {
    fn load(&self) -> Result<Option<Credentials>> {
        match self.entry()?.get_password() {
            Ok(json) => {
                let s: Secret = serde_json::from_str(&json).map_err(store_error)?;
                Ok(Some(Credentials::new(s.email, s.password)))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(store_error(e)),
        }
    }

    fn save(&self, c: &Credentials) -> Result<()> {
        let json = serde_json::to_string(&Secret {
            email: c.email.clone(),
            password: c.password.clone(),
        })
        .map_err(store_error)?;
        self.entry()?.set_password(&json).map_err(store_error)
    }

    fn clear(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(store_error(e)),
        }
    }
}

/// Credentials held in memory; for tests and demos.
#[derive(Default)]
pub struct MemoryCredentials(Mutex<Option<Credentials>>);

impl MemoryCredentials {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// A store that already holds `credentials`.
    pub fn with(credentials: Credentials) -> Self {
        Self(Mutex::new(Some(credentials)))
    }
}

impl CredentialStore for MemoryCredentials {
    fn load(&self) -> Result<Option<Credentials>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, c: &Credentials) -> Result<()> {
        *self.0.lock().unwrap() = Some(c.clone());
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

/// Reads from `primary` first, then `fallback`; writes only go to `fallback`.
///
/// The standard setup is environment variables (read-only) over the keychain.
pub struct LayeredCredentials {
    primary: Box<dyn CredentialStore>,
    fallback: Box<dyn CredentialStore>,
}

impl LayeredCredentials {
    /// Layers `primary` over `fallback`.
    pub fn new(primary: Box<dyn CredentialStore>, fallback: Box<dyn CredentialStore>) -> Self {
        Self { primary, fallback }
    }

    /// Environment variables over the OS keychain.
    pub fn standard() -> Self {
        Self::new(
            Box::new(EnvCredentials::from_process()),
            Box::new(KeyringCredentials::new()),
        )
    }
}

impl CredentialStore for LayeredCredentials {
    fn load(&self) -> Result<Option<Credentials>> {
        match self.primary.load()? {
            Some(c) => Ok(Some(c)),
            None => self.fallback.load(),
        }
    }

    fn save(&self, c: &Credentials) -> Result<()> {
        self.fallback.save(c)
    }

    fn clear(&self) -> Result<()> {
        self.fallback.clear()
    }
}
