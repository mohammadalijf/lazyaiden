# lazyaiden-profiles

Business logic for brew profiles.

* `FsProfileStore`: one `<name>.yaml` per profile (atomic writes); remote links in `.lazyaiden-links.yaml`, per brewer.
* `ProfileService`: `list_local`, `create_local`, `update_local`, `import_str/file`, `export_str/file`, `status`, `diff`,
  `push`/`push_all` (create, update or no-op; never duplicates), `pull`/`pull_all`, `delete_remote`, `share_link`,
  `import_from_link`.
* The brewer protocol and the store are injected (`Arc<dyn FellowApi>`, `Arc<dyn ProfileStore>`).

File format: [docs/profile-format.md](../../docs/profile-format.md). Tests run against `fellow_client::testing::InMemoryFellow`
and temporary directories: `cargo test -p lazyaiden-profiles`.
