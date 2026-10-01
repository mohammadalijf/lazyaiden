# TUI guide (`lazyaiden`)

```
lazyaiden [--config FILE] [--profiles-dir DIR] [--brewer ID] [--demo]
```

Layout (LazyGit style): three stacked panels on the left (**1 Profiles**, **2 Schedules**, **3 Device**), a
details pane on the right, and a key-hint / message bar at the bottom. The focused panel has a green border;
the details pane follows the selection.

## Profile badges

| Badge | State |
| --- | --- |
| `✓` | in sync with the brewer |
| `~` | modified: local differs (the details pane lists the differing fields) |
| `+` | local only |
| `↓` | on the brewer only (pull to edit) |
| `!` | linked but missing on the brewer |
| `·` | unknown (offline / logged out) |

After the badge, a tag shows who made the brewer copy: `[C]` created by you (remote id `p<n>`), `[F]` shipped
by Fellow (`d<n>` or `plocal<n>`). Fellow profiles are hidden by default; the panel title counts them and `f`
shows or hides them.

The status bar shows the active brewer, `offline` when the brewer cannot be reached, and a spinner naming the
running request (`⠋ pushing morning-v60…`). Until the first load finishes, the panels show `loading…`.
Messages clear on the next key press.

## Keys

| Key | Action |
| --- | --- |
| `Tab` / `Shift-Tab`, `1` `2` `3` | switch panel |
| `j` `k` `↑` `↓`, `g` `G` | move, first, last |
| `r` | refresh from the brewer |
| `b` | choose brewer |
| `?` | help |
| `q`, `Ctrl-C` | quit (`Ctrl-C` works everywhere) |

**Profiles**

| Key | Action |
| --- | --- |
| `n` | new profile from a template, then opens the form |
| `e` / `Enter` | edit in a form |
| `E` | edit the YAML in `$VISUAL`/`$EDITOR`; validated when you return |
| `i` / `x` | import / export a YAML or JSON file (prompts for a path) |
| `p` / `P` | push to / pull from the brewer (`P` asks first if it would overwrite local changes) |
| `U` / `D` | push all / pull all (with confirmation) |
| `d` | delete: local only, or both (`b`oth / `l`ocal / `n`o) for linked profiles; remote-only profiles delete on the brewer |
| `s` | create a share link and copy it to the clipboard (also shown in the status bar) |
| `f` | show / hide profiles made by Fellow |
| `L` | import from a share link |

**Schedules**: `n` new (needs profiles on the brewer), `t`/`Enter` toggle, `d` delete (confirmation).

**Device**: `Enter` opens the brewer picker.

## Forms

`Tab`/`↓` next field, `↑`/`Shift-Tab` previous, `Space`/`←`/`→` toggle booleans (or change the schedule's
profile), `Ctrl-U` clears a field, `Ctrl-S` saves, `Esc` cancels. Every edit is validated live; fields with
problems show the reason and allowed range, and saving is blocked until the form is valid.

## Brewer picker

With several brewers and none chosen, the picker opens automatically after the first load. `↑`/`↓` then
`Enter` selects and remembers the choice in the config file; `Esc` skips (remote features stay unavailable).
`b` reopens it at any time.

## Logged out

Without credentials, local profiles still list and edit; the status bar shows `offline` and remote actions
explain how to log in (`lazyaiden-cli login`).
