# Architecture

## Layers

```
apps/lazyaiden-tui   apps/lazyaiden-cli          presentation: parse input, render output
        \            /
        crates/lazyaiden-core                composition root: config, credentials, wiring
        /           \
lazyaiden-profiles   lazyaiden-schedules         business logic (depend on the *protocol* only)
        \           /
        crates/fellow-client             protocol (FellowApi) + default HTTP implementation
```

Rules that keep it honest:

1. **Apps never use the client.** They get an `lazyaiden_core::Aiden` with `profiles`, `schedules` and `devices`
   services. `lazyaiden-core` is the only crate that builds an `HttpFellowClient`.
2. **Business logic depends on the trait, not the implementation.** `ProfileService`, `ScheduleService` and
   `DeviceService` hold an `Arc<dyn FellowApi>`; `ProfileService` also holds an `Arc<dyn ProfileStore>`.
3. **Presentation does no I/O in its core logic.** The TUI is a pure reducer plus a renderer; the CLI's
   `run` function takes injectable streams.

## Dependency injection

```rust
// the single place that picks implementations (lazyaiden-core/src/aiden.rs)
let api: Arc<dyn FellowApi> = Arc::new(HttpFellowClient::new(creds, options)?);
let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
let aiden = Aiden::from_parts(api, store, config);   // services are wired here
```

`Aiden::from_parts` is the seam: tests pass `InMemoryFellow`; `Aiden::demo` (feature `demo`) passes a seeded
fake so the apps run offline (`--demo`); `Aiden::connect` falls back to `LoggedOut`, a `FellowApi` whose
every remote call fails with a clear "not logged in" error, so local-only features still work without credentials.

### Adding another `FellowApi` implementation

Implement `fellow_client::FellowApi` (13 methods) and pass it to `Aiden::from_parts`. To prove it behaves, run
the contract suite in `crates/fellow-client/tests/contract.rs` against it: it is written against
`&dyn FellowApi`, and already runs against both the in-memory fake and the HTTP client.

## Brewers (multi-device)

The server scopes profiles and schedules per brewer. `FellowApi` keeps a *selected brewer*:

* exactly one brewer on the account: selected automatically;
* several and none chosen: device-scoped calls fail with `FellowError::BrewerNotSelected(devices)`, which
  carries the candidates (the TUI opens its picker from that error);
* the choice is persisted as `brewer_id` in the config file by `Aiden::use_brewer`.

`active_brewer()` resolves the effective brewer (including the automatic one); `ProfileService` uses it to key
remote links per brewer.

## Local profiles and linking

A profile is `<profiles_dir>/<name>.yaml`; the file stem is its local identity. Which remote profile a file
corresponds to is recorded **per brewer** in the sidecar `<profiles_dir>/.lazyaiden-links.yaml`:

```yaml
b1:                # brewer id
  morning-v60: p12 # local name -> remote profile id
```

The sidecar keeps profile files pristine (no tool-generated fields, comments survive pushes, diffs stay clean) and
lets one local profile be pushed to several brewers. It is account-specific, so you may want to add it to
`.gitignore` if you version the profiles directory.

Push semantics (`ProfileService::push`): validate -> look up the linked remote id -> **create** if unlinked or
the remote is gone, **update** if the data differs, **nothing** if identical. Pull is the mirror image and never
creates duplicates for already-linked profiles.

## Validation

Validation lives with the data (`ProfileDraft::issues`, `ScheduleDraft::issues` in `fellow-client`) and is
enforced at every boundary:

| Boundary | Where |
| --- | --- |
| CLI arguments | clap parsers + `Days`/`TimeOfDay` parsing |
| TUI forms | live, per field, using the same `issues()` |
| Services | before any network call (`check`, `NewSchedule` checks) |
| Client | `create_*`/`update_*` validate before sending |
| Hand-edited files | `status`/`push`/`edit` re-validate and report field names and ranges |

## Error handling

Each layer has one error enum (`FellowError`, `ProfileError`/`StoreError`, `ScheduleError`, `CoreError`,
`CliError`); wrappers use `#[error(transparent)]`, so messages are never doubled. The CLI maps them to
[exit codes](cli.md#exit-codes). Note that `transparent` hides the inner error from `source()`; code that needs
the underlying `FellowError` (the TUI's brewer picker) unwraps the known wrapper types explicitly.

## TUI structure

`App::update(Event) -> Vec<Effect>` is a pure reducer. `effects::perform` executes an effect against the
services and returns exactly one `Msg`; the runtime feeds it back. A busy counter (effects emitted minus
messages received) drives the spinner and is asserted to return to zero in every flow test. See [tui.md](tui.md).
