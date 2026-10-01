# Fellow API notes

Reverse-engineered from the official app's traffic as documented by
[`9b/fellow-aiden`](https://github.com/9b/fellow-aiden) (Python),
[`simmerkaer/fellow-aiden-ts`](https://github.com/simmerkaer/fellow-aiden-ts) and
[`simmerkaer/fellow-aiden-dotnet`](https://github.com/simmerkaer/fellow-aiden-dotnet). **Unofficial and
unversioned**: expect it to change. The default implementation is `HttpFellowClient` in
[`crates/fellow-client`](../crates/fellow-client).

* Base URL: `https://l8qtmnc692.execute-api.us-west-2.amazonaws.com/v1`
* Header `User-Agent: Fellow/5 CFNetwork/1568.300.101 Darwin/24.2.0` (required), `Authorization: Bearer <accessToken>`.
* Retries: 408, 500, 502, 503, 504 and network errors, with linear back-off (3 retries). On `401` the client logs in
  again once and retries.

| Operation | Request | Notes |
| --- | --- | --- |
| Login | `POST /auth/login` `{email,password}` | returns `{accessToken, refreshToken}` |
| Devices | `GET /devices?dataType=real` | array; each has `id`, `displayName`, ... |
| Device setting | `PATCH /devices/{id}` `{setting: value}` | |
| List profiles | `GET /devices/{id}/profiles` | |
| Create profile | `POST /devices/{id}/profiles` | body = the 15 editable fields |
| Update profile | `PATCH /devices/{id}/profiles/{pid}` | |
| Delete profile | `DELETE /devices/{id}/profiles/{pid}` | |
| Share profile | `POST /devices/{id}/profiles/{pid}/share` | returns `{link}` |
| Shared profile | `GET /shared/{brewId}` | brew id = last path segment of the link |
| List schedules | `GET /devices/{id}/schedules` | |
| Create schedule | `POST /devices/{id}/schedules` | `{days[7], secondFromStartOfTheDay, enabled, amountOfWater, profileId}` |
| Toggle schedule | `PATCH /devices/{id}/schedules/{sid}` `{enabled}` | |
| Delete schedule | `DELETE /devices/{id}/schedules/{sid}` | |

Server-managed profile fields (`id, createdAt, deletedAt, lastUsedTime, sharedFrom, isDefaultProfile,
instantBrew, folder, duration, lastGBQuantity`) are never sent; they are ignored when a server profile is
converted to the editable `ProfileDraft`. Unknown fields are preserved in the `extra` map of `Profile`, `Schedule` and
`Device`.

Schedule ids reference profiles as `p<number>` (cloud) or `plocal<number>` (on-device). Creating a schedule for a
profile that does not exist yields a 400 ("Profile could not be found").

There is no documented "brew now" endpoint, so this project only manages recurring schedules.

Value constraints are in [profile-format.md](profile-format.md). The references assume one brewer per account;
this project supports several (see [architecture](architecture.md#brewers-multi-device)).
