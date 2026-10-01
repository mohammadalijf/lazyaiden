# Development

```bash
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features        # unit, integration, snapshot and doc tests
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

CI (`.github/workflows/ci.yml`) runs the same commands.

## Test strategy

| Crate / app | What is tested |
| --- | --- |
| `fellow-client` | model serde round trips (unknown fields kept), every validation boundary, wiremock tests for login, bearer header, re-auth (once, no loop), retries/back-off, body shapes, path encoding, brewer selection; a **contract suite** run against `InMemoryFellow` *and* against `HttpFellowClient` talking to a wiremock server backed by that fake |
| `lazyaiden-profiles` | store (atomic writes, malformed YAML with line numbers, name safety, sidecar links), slug properties (proptest), the full service: import/export, push (create/update/unchanged, no duplicates), pull, status, diff, per-brewer links |
| `lazyaiden-schedules` | time/day parsing (examples + proptest round trips), service rules |
| `lazyaiden-core` | config precedence (flag > env > file > default), credential layering, device resolution, wiring, login verification (wiremock), degraded "logged out" mode |
| `lazyaiden-cli` | in-process end-to-end runs sharing one fake backend (insta snapshots, exit codes), real-binary tests with `assert_cmd` |
| `lazyaiden-tui` | form/prompt state machines, the reducer, full flows (real reducer + real effects over the fake), `TestBackend` render snapshots, tiny-terminal safety |

### Snapshots

Snapshot tests use [insta](https://insta.rs). After an intentional UI/output change:

```bash
INSTA_UPDATE=always cargo test -p lazyaiden-cli -p lazyaiden-tui
git diff -- '*.snap'            # review every changed snapshot
```

### Fixtures

`examples/profiles/*.yaml` are both user-facing samples and test fixtures.

### Keychain in tests

No test touches the real OS keychain: binary tests set dummy `FELLOW_*` variables, everything else uses
`MemoryCredentials`.

## Adding an endpoint

1. Add a method to `FellowApi` (`crates/fellow-client/src/api.rs`).
2. Implement it in `HttpFellowClient` (`http.rs`) and `InMemoryFellow` (`testing.rs`); add the route to
   `tests/common/fake_server.rs`.
3. Extend the contract suite so both implementations are held to the same behaviour.
4. Use it from a service in `lazyaiden-profiles` / `lazyaiden-schedules` / `lazyaiden-core`, never directly from an app.
5. Update [fellow-api.md](fellow-api.md).

## Conventions

* Libraries use `#![warn(missing_docs)]`; clippy runs with `-D warnings`.
* No `unsafe` (`forbid` workspace-wide).
* Services take `Arc<dyn Trait>` collaborators; constructors never read the environment or the network.
