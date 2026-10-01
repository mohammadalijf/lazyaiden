# fellow-client

Client for the (unofficial, reverse-engineered) Fellow Aiden cloud API.

- **Protocol**: the `FellowApi` trait — login, brewers, profiles, schedules, sharing, settings.
- **Default implementation**: `HttpFellowClient` (reqwest + rustls). Lazy login, one re-login on `401`,
  retries on network errors and 408/500/502/503/504 with linear back-off.
- **Models**: `ProfileDraft` / `ScheduleDraft` are the editable, validated shapes; `Profile` / `Schedule` /
  `Device` are lenient server shapes that keep unknown fields.
- **Validation**: `ProfileDraft::validate()` and `ScheduleDraft::validate()` enforce the allowed values
  (see `validation.rs`). The client validates before every create/update, so bad data never leaves the process.
- **Testing**: enable the `testing` feature for `testing::InMemoryFellow`, an in-memory `FellowApi`.

## Brewer selection

Device-scoped calls act on the *selected brewer*. A single-brewer account is auto-selected; otherwise
call `select_brewer(id)` or you get `FellowError::BrewerNotSelected` listing the candidates.

## Tests

`cargo test -p fellow-client` runs unit tests, wiremock-based HTTP tests (auth, retry, bodies, paths) and a
**contract suite** executed against both `InMemoryFellow` and `HttpFellowClient` talking to a wiremock server
backed by that same fake — so the fake cannot drift from the real client's behaviour.

See [`docs/fellow-api.md`](../../docs/fellow-api.md) for the endpoint reference.
