use std::sync::{Arc, Mutex};

use fellow_client::{Device, FellowApi, HttpClientOptions, HttpFellowClient};
use lazyaiden_profiles::{FsProfileStore, ProfileService, ProfileStore};
use lazyaiden_schedules::ScheduleService;

use crate::config::Config;
use crate::credentials::{CredentialStore, Credentials};
use crate::devices::DeviceService;
use crate::error::Result;
use crate::logged_out::LoggedOut;

/// The wired-up application services.
pub struct Aiden {
    /// Local and remote profile management.
    pub profiles: ProfileService,
    /// Brew schedule management.
    pub schedules: ScheduleService,
    /// Brewer listing and selection.
    pub devices: DeviceService,
    config: Mutex<Config>,
    /// The throwaway config and profiles directory of a demo instance; deleted on drop.
    #[cfg(feature = "demo")]
    _demo_dir: Option<tempfile::TempDir>,
}

fn http_client(config: &Config, credentials: Credentials) -> Result<HttpFellowClient> {
    let mut options = HttpClientOptions::default();
    if let Some(url) = &config.base_url {
        options.base_url = url.clone();
    }
    Ok(HttpFellowClient::new(credentials, options)?)
}

impl Aiden {
    /// Wires the services around the given protocol implementation and store.
    ///
    /// This is the dependency-injection seam: tests and alternative backends
    /// pass their own `api`. The configured brewer, if any, is selected.
    pub fn from_parts(
        api: Arc<dyn FellowApi>,
        store: Arc<dyn ProfileStore>,
        config: Config,
    ) -> Self {
        if let Some(id) = &config.brewer_id {
            api.select_brewer(id);
        }
        Self {
            profiles: ProfileService::new(api.clone(), store),
            schedules: ScheduleService::new(api.clone()),
            devices: DeviceService::new(api),
            config: Mutex::new(config),
            #[cfg(feature = "demo")]
            _demo_dir: None,
        }
    }

    /// Builds the default setup: the HTTP client with stored credentials and the
    /// YAML profile directory from `config`. Performs no network I/O.
    ///
    /// Without credentials (or when the credential store cannot be read) the
    /// remote side is [`LoggedOut`]: local profile operations work, remote ones
    /// fail with an authentication error that explains how to log in.
    pub fn connect(config: Config, credentials: &dyn CredentialStore) -> Result<Self> {
        let api: Arc<dyn FellowApi> = match credentials.load() {
            Ok(Some(creds)) => Arc::new(http_client(&config, creds)?),
            Ok(None) => Arc::new(LoggedOut::default()),
            // An unreadable credential store must not break local-only features.
            Err(e) => Arc::new(LoggedOut::because(e.to_string())),
        };
        let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
        Ok(Self::from_parts(api, store, config))
    }

    /// A fully offline instance backed by an in-memory brewer with sample data,
    /// for trying the apps without an account.
    ///
    /// It never touches the user's files: the config lives in a temporary directory
    /// that is deleted when the instance is dropped, and so do the local profiles
    /// unless `profiles_dir` names a directory to keep them in (so separate demo runs
    /// can share local files).
    #[cfg(feature = "demo")]
    pub async fn demo(profiles_dir: Option<std::path::PathBuf>) -> Result<Self> {
        use fellow_client::ScheduleDraft;
        use fellow_client::testing::InMemoryFellow;

        let demo_error = |path: &std::path::Path, message: String| crate::CoreError::Config {
            path: path.into(),
            message: format!("cannot set up the demo: {message}"),
        };
        let dir = tempfile::Builder::new()
            .prefix("lazyaiden-demo-")
            .tempdir()
            .map_err(|e| demo_error(&std::env::temp_dir(), e.to_string()))?;
        let config = Config {
            path: dir.path().join("config.toml"),
            profiles_dir: profiles_dir.unwrap_or_else(|| dir.path().join("profiles")),
            base_url: None,
            brewer_id: None,
        };
        let fake = Arc::new(InMemoryFellow::with_brewers(&[(
            "demo-brewer",
            "Aiden (demo)",
        )]));
        let mut everyday = None;
        for (title, ratio) in [("Demo Everyday", 16.0), ("Demo Light Roast", 15.5)] {
            let mut draft = lazyaiden_profiles::template(title);
            draft.ratio = ratio;
            let p = fake
                .create_profile(&draft)
                .await
                .expect("demo profile is valid");
            everyday.get_or_insert(p.id);
        }
        fake.create_schedule(&ScheduleDraft {
            days: [false, true, true, true, true, true, false],
            second_from_start_of_the_day: 7 * 3600 + 30 * 60,
            enabled: true,
            amount_of_water: 500,
            profile_id: everyday.expect("seeded above"),
        })
        .await
        .expect("demo schedule is valid");
        let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
        let mut aiden = Self::from_parts(fake, store, config);
        let iced = lazyaiden_profiles::template("Demo Iced");
        if aiden.profiles.get_local(&iced.title).is_err() {
            aiden
                .profiles
                .create_local(iced)
                .map_err(|e| demo_error(dir.path(), e.to_string()))?;
        }
        aiden._demo_dir = Some(dir);
        Ok(aiden)
    }

    /// A snapshot of the resolved configuration.
    pub fn config(&self) -> Config {
        self.config.lock().expect("config lock").clone()
    }

    /// Selects a brewer (by id or name) and remembers the choice in the config file.
    pub async fn use_brewer(&self, query: &str) -> Result<Device> {
        let device = self.devices.select(query).await?;
        self.config
            .lock()
            .expect("config lock")
            .persist_brewer(&device.id)?;
        Ok(device)
    }
}

/// Verifies `credentials` against Fellow and, only if they work, stores them.
pub async fn login(
    config: &Config,
    credentials: Credentials,
    store: &dyn CredentialStore,
) -> Result<()> {
    let client = http_client(config, credentials.clone())?;
    client.login().await?;
    store.save(&credentials)
}

/// Forgets the stored credentials.
pub fn logout(store: &dyn CredentialStore) -> Result<()> {
    store.clear()
}
