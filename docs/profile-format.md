# Profile format

A profile is a YAML (or JSON) document whose keys match the Fellow API's camelCase names. All keys are required.
Unknown keys (for example `id` or `createdAt` in a profile copied from the API) are ignored.

```yaml
profileType: 0
title: Morning V60
ratio: 16.0              # 1:x brew ratio
bloomEnabled: true
bloomRatio: 2.0
bloomDuration: 35        # seconds
bloomTemperature: 96.0   # °C
ssPulsesEnabled: true    # single-serve pulses
ssPulsesNumber: 3
ssPulsesInterval: 20     # seconds
ssPulseTemperatures: [96.0, 95.5, 95.0]
batchPulsesEnabled: true
batchPulsesNumber: 3
batchPulsesInterval: 25
batchPulseTemperatures: [96.0, 95.5, 95.0]
```

More samples: [`examples/profiles/`](../examples/profiles). They are also used as test fixtures.

## Fields and allowed values

| Key | Type | Allowed |
| --- | --- | --- |
| `profileType` | integer | `0` for regular profiles (kept as is) |
| `title` | string | 1-50 chars: ASCII letters, digits, space and `! @ # $ % & * - + ? / . , : ) (` |
| `ratio` | number | 14 to 20 in steps of 0.5 |
| `bloomEnabled` | bool | |
| `bloomRatio` | number | 1 to 3 in steps of 0.5 |
| `bloomDuration` | integer | 1 to 120 (seconds) |
| `bloomTemperature` | number | 50 to 98.5 in steps of 0.5 (°C) |
| `ssPulsesEnabled` | bool | |
| `ssPulsesNumber` | integer | 1 to 10 |
| `ssPulsesInterval` | integer | 5 to 60 (seconds) |
| `ssPulseTemperatures` | list of numbers | each 50 to 98.5 in steps of 0.5 |
| `batchPulsesEnabled` | bool | |
| `batchPulsesNumber` | integer | 1 to 10 |
| `batchPulsesInterval` | integer | 5 to 60 (seconds) |
| `batchPulseTemperatures` | list of numbers | each 50 to 98.5 in steps of 0.5 |

These rules come from the reference implementations. The length of the temperature lists is **not** checked
against the pulse counts (the references do not either).

Invalid files are never pushed. They are reported with the field and range, e.g.
`invalid profile: ratio: 99 is not one of 14–20 in steps of 0.5`, and `lazyaiden-cli profile list --local` /
the TUI show unreadable files instead of hiding them.

## File naming

The file stem is the profile's local name: `morning-v60.yaml` -> `morning-v60`. New profiles get a slug of
the title (`Morning V60` -> `morning-v60`, then `-2`, `-3`... on collisions). Commands accept the file name or the
title (case-insensitive; ambiguous titles must be addressed by file name). `.yaml` and `.yml` are both read.
Files starting with `.` are ignored (`.lazyaiden-links.yaml` is the link sidecar).

## Working with git or other editors

* Point `--profiles-dir` / `LAZYAIDEN_PROFILES_DIR` / `profiles_dir` at a checkout.
* Editing files directly is fine; lazyaiden only rewrites a profile file when *you* change it through the TUI form,
  `pull`, `import --overwrite` or `new`. `push` never touches it.
* Import and export are pristine documents: `profile export` writes only the fields above, so exported files can be
  shared or committed anywhere, and `profile add -i` accepts them back (or a JSON profile fetched from the API).
* The link sidecar `.lazyaiden-links.yaml` maps local names to remote ids per brewer; ignore it in git if the
  directory is shared between accounts.
