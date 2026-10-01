# lazyaiden-schedules

Business logic for recurring brew schedules.

* `Days` parses `mon,wed,fri`, `mon-fri`, `fri-mon`, `weekdays`, `weekends`, `daily`; `TimeOfDay` parses `07:30`, `7:30am`, `12pm`.
  Both format back to something they can re-parse (property-tested).
* `ScheduleService`: `list` (with profile titles), `create` (resolves a profile id or title, validates), `toggle`,
  `set_enabled`, `delete`. The brewer protocol is injected as `Arc<dyn FellowApi>`.

`cargo test -p lazyaiden-schedules`
