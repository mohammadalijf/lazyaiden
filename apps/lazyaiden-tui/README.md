# lazyaiden (TUI)

LazyGit-style terminal UI. Guide and key map: [docs/tui.md](../../docs/tui.md). Try it offline with `cargo run -p lazyaiden-tui -- --demo`.

Structure: pure reducer (`state`), effect runner (`effects`), renderer (`ui`), terminal loop (`runtime`).
`cargo test -p lazyaiden-tui`
