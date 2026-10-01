# CLI reference (`lazyaiden-cli`)

```
lazyaiden-cli [OPTIONS] <COMMAND>
```

Global options: `--config <FILE>`, `--profiles-dir <DIR>`, `--brewer <ID>`, `--json`, `--demo`
(`--base-url` is hidden, for testing). See the [configuration table](../README.md#configuration).

Commands that only touch local files (`profile list --local|show|add|new|export|edit`, `rm` without
`--remote`) work **without logging in**. Everything else needs credentials.

## Authentication

| Command | Description |
| --- | --- |
| `login [--email E] [--password-stdin]` | Verifies the credentials against Fellow, then stores them in the OS keychain. The password is prompted (hidden) or read from the first line of stdin; it is never a command-line argument. |
| `logout` | Removes the stored credentials. |

`FELLOW_EMAIL` / `FELLOW_PASSWORD` override the keychain for every command.

## Devices

| Command | Description |
| --- | --- |
| `device list` | Brewers on the account; `*` marks the active one. |
| `device use <ID\|NAME>` | Selects a brewer and saves `brewer_id` in the config file. |
| `device info` | The active brewer's details. |

With several brewers and none selected, remote commands fail with exit code 5 and list the candidates.

## Profiles

| Command | Description |
| --- | --- |
| `profile list [--local]` | Name, title, sync state and remote id. `--local` skips the network. |
| `profile show <P>` | The profile as YAML. |
| `profile add -i <FILE\|-> [--name N] [--overwrite]` | Headless import of YAML/JSON. Validates first. Fails with exit 6 if a profile with that name/title exists, unless `--overwrite`. |
| `profile new <TITLE>` | Creates a local profile from a template. |
| `profile export <P> [-o FILE\|-]` | Writes pristine YAML (stdout by default). |
| `profile edit <P>` | Opens `$VISUAL`/`$EDITOR` (default `vi`), then validates; an invalid result exits 4 and tells you to edit again. |
| `profile rm <P> [--remote --yes]` | Deletes the local file; with `--remote` also the brewer's copy (needs `--yes`). A failed remote delete keeps the local file. |
| `profile push <P> \| --all` | Creates or updates on the brewer; never duplicates. |
| `profile pull <ID\|TITLE> \| --all` | Fetches into local files, keeping links so later pulls update the same file. |
| `profile diff <P>` | Fields that differ between local and remote. |
| `profile share <P>` | Prints a share link (profile must be pushed). |
| `profile import-link <URL\|ID>` | Imports a shared profile as a local file (not pushed). |

`<P>` is a file name or a title (case-insensitive).

Sync states: `local-only`, `in-sync`, `modified` (local differs), `remote-missing` (linked but deleted on
the brewer), `remote-only`.

## Schedules

| Command | Description |
| --- | --- |
| `schedule list` | Id, enabled, days, time, water, profile. |
| `schedule add --profile <ID\|TITLE> --days D --time T --water ML [--disabled]` | Creates a schedule. |
| `schedule toggle <ID> [--on\|--off]` | Flips, or forces, the enabled flag. |
| `schedule rm <ID>` | Deletes a schedule. |

* `--days`: `mon,wed,fri`, `mon-fri`, `fri-mon` (wraps), `weekdays`, `weekends`, `daily`; combinable (`mon-wed,sat`).
* `--time`: `07:30`, `7:30`, `07:30:15`, `7:30am`, `12pm`.
* `--water`: 150-1500 ml.

A schedule is a recurring on-device brew; the API offers no one-off "brew now".

## JSON output

`--json` prints one JSON document per command instead of text, for example:

```json
{ "profiles": [ { "name": "morning-v60", "title": "Morning V60", "state": "in-sync", "remoteId": "p12" } ],
  "problems": [] }
```
```json
{ "results": [ { "name": "morning-v60", "action": "created", "remoteId": "p12" },
               { "name": "bad", "error": "invalid profile: ratio: 99 is not one of 14–20 in steps of 0.5" } ] }
```

Warnings (e.g. unreadable files) go to stderr; stdout stays parseable.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | success |
| 1 | other failure (I/O, config, or a batch with failures already listed in the output) |
| 2 | usage error |
| 3 | authentication (bad or missing credentials, credential store) |
| 4 | invalid data (validation, unparsable YAML/time/days) |
| 5 | not found, ambiguous, not yet pushed, or brewer not selected |
| 6 | conflict (profile already exists) |
| 7 | network or server error |

`push --all` / `pull --all` exit 1 if any item failed but still process the rest.

## Shell completions

`lazyaiden-cli completions <bash|zsh|fish|elvish|powershell>` prints a script, e.g.
`lazyaiden-cli completions zsh > ~/.zfunc/_aiden-cli`.

## Manual pages

`cargo install` does not install man pages, so write them once (and again after upgrading):

```
lazyaiden-cli man --dir ~/.cargo/share/man/man1
```

`man` looks in `../share/man` next to every `bin` directory on `PATH`, so `man lazyaiden-cli` and
`man lazyaiden-cli-profile-push` then work without setting `MANPATH`. Without `--dir`, `lazyaiden-cli man` prints the
top-level page to stdout.
