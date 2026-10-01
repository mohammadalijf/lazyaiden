# lazyaiden-cli

Headless front end: `lazyaiden-cli profile add -i profile.yaml`, `profile push --all`, `schedule add ...`, `--json` output and
stable exit codes. Full reference: [docs/cli.md](../../docs/cli.md).

The library (`lazyaiden_cli`) exposes `Cli`, `execute` and `run` so the whole tool is testable in-process.
`cargo test -p lazyaiden-cli`
