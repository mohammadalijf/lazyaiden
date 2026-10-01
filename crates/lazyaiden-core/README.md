# lazyaiden-core

The composition root. The only crate that builds the HTTP client.

* `Config::resolve`: `flag > env > config file > default` (`LAZYAIDEN_CONFIG`, `LAZYAIDEN_PROFILES_DIR`, `LAZYAIDEN_BREWER_ID`, `LAZYAIDEN_BASE_URL`).
* `CredentialStore`: `EnvCredentials` (read-only), `KeyringCredentials` (OS keychain), `LayeredCredentials`, `MemoryCredentials`.
* `DeviceService`: list brewers, resolve by id or name, select the active one.
* `Aiden`: `connect` (HTTP, or `LoggedOut` when no credentials), `from_parts` (inject anything), `demo` (feature `demo`,
  offline sample data), `use_brewer` (select and persist).
* `login` verifies credentials before storing them.

`cargo test -p lazyaiden-core --all-features`
