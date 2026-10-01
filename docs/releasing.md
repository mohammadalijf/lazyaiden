# Releasing

Releases are driven by git tags. Pushing `vX.Y.Z` runs [`release.yml`](../.github/workflows/release.yml),
which:

1. checks that the tag matches `workspace.package.version` in `Cargo.toml` and runs the tests;
2. renders the man pages and shell completions once (`cargo xtask dist-assets`);
3. builds `lazyaiden` and `lazyaiden-cli` for `aarch64`/`x86_64` macOS and Linux and packs one archive
   per target, `lazyaiden-vX.Y.Z-<target>.tar.gz` (both binaries, `man/`, `completions/`, `LICENSE`,
   `README.md`) plus a `.sha256` file;
4. publishes a GitHub release with notes generated from the merged PRs;
5. renders [`packaging/homebrew/lazyaiden.rb.in`](../packaging/homebrew/lazyaiden.rb.in) and pushes it to
   [`mohammadalijf/homebrew-tap`](https://github.com/mohammadalijf/homebrew-tap).

## Cutting a release

```bash
cargo set-version 0.2.0          # cargo install cargo-edit; or edit Cargo.toml by hand
cargo check                      # refreshes Cargo.lock
git commit -am "Release 0.2.0"   # via a PR, like any other change
git tag v0.2.0                   # on main, after the PR is merged
git push origin v0.2.0
```

The version lives only in the root `Cargo.toml`; every crate inherits it.

## Release candidates

Tag a semver prerelease such as `v0.2.0-rc.1` (with `version = "0.2.0-rc.1"` in `Cargo.toml`). The same
pipeline runs, but:

* the GitHub release is marked **pre-release**, so it never becomes "Latest";
* the tap gets `Formula/lazyaiden-rc.rb` instead of touching `lazyaiden.rb`, so only testers who opt in
  get it: `brew install mohammadalijf/tap/lazyaiden-rc`. The two formulae conflict; switch with
  `brew uninstall lazyaiden-rc && brew install lazyaiden`.

To promote, set the version to `0.2.0` and tag `v0.2.0`. Its notes cover everything since the previous
stable release, not just the changes since the last RC.

## Release notes

Notes are generated from merged PR titles, grouped by label (see [`.github/release.yml`](../.github/release.yml)):
`breaking`, `feature`/`enhancement`, `bug`/`fix`, `documentation`; anything else lands in "Other changes",
and `skip-changelog` hides a PR. Labels only shape the notes; they never trigger a release.

## Re-running a failed release

Re-run the failed jobs from the Actions tab. If the GitHub release already exists, its assets are replaced
and its notes kept; the tap commit is skipped when the formula is unchanged. If the tag itself is wrong,
delete the release and the tag, fix, and tag again.

## Secrets

`HOMEBREW_TAP_TOKEN`: a fine-grained personal access token with **Contents: read and write** on
`homebrew-tap` only. When it expires the `homebrew` job fails; the GitHub release is unaffected. Rotate the
token, then re-run the job.

## Checking the assets locally

```bash
cargo xtask dist-assets /tmp/lazyaiden-dist
man /tmp/lazyaiden-dist/man/lazyaiden-cli.1
packaging/homebrew/render.sh 0.2.0 <dir with .sha256 files>
```
