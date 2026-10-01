#![allow(dead_code)]

use std::io;
use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use fellow_client::testing::InMemoryFellow;
use lazyaiden_cli::{Cli, Io, run};
use lazyaiden_core::profiles::FsProfileStore;
use lazyaiden_core::{Aiden, Config, Overrides};
use tempfile::TempDir;

pub const MORNING: &str = include_str!("../../../../examples/profiles/morning-v60.yaml");
pub const LIGHT: &str = include_str!("../../../../examples/profiles/light-roast-batch.yaml");

pub struct Out {
    pub code: u8,
    pub out: String,
    pub err: String,
}

type Editor = Box<dyn Fn(&Path) -> io::Result<bool>>;

/// One `Aiden` over an in-memory brewer, shared by every command run through it.
pub struct Harness {
    pub dir: TempDir,
    pub fake: Arc<InMemoryFellow>,
    pub aiden: Aiden,
    pub editor: Editor,
}

impl Harness {
    pub fn new() -> Self {
        Self::with_fake(InMemoryFellow::new())
    }

    pub fn with_fake(fake: InMemoryFellow) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::resolve(
            &Overrides {
                config_path: Some(dir.path().join("config.toml")),
                profiles_dir: Some(dir.path().join("profiles")),
                ..Overrides::default()
            },
            &|_| None,
        )
        .unwrap();
        let fake = Arc::new(fake);
        let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
        let aiden = Aiden::from_parts(fake.clone(), store, config);
        Self {
            dir,
            fake,
            aiden,
            editor: Box::new(|_| Ok(true)),
        }
    }

    pub fn profiles_dir(&self) -> std::path::PathBuf {
        self.dir.path().join("profiles")
    }

    pub fn write(&self, rel: &str, text: &str) -> std::path::PathBuf {
        let p = self.dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    pub async fn run(&mut self, args: &[&str]) -> Out {
        self.run_with_stdin(args, "").await
    }

    pub async fn run_with_stdin(&mut self, args: &[&str], stdin: &str) -> Out {
        let argv = std::iter::once("lazyaiden-cli").chain(args.iter().copied());
        let cli = match Cli::try_parse_from(argv) {
            Ok(c) => c,
            Err(e) => {
                return Out {
                    code: 2,
                    out: String::new(),
                    err: e.to_string(),
                };
            }
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut input = stdin.as_bytes();
        let mut io = Io {
            out: &mut out,
            err: &mut err,
            stdin: &mut input,
            editor: &*self.editor,
        };
        let result = run(&cli.command, cli.json, &self.aiden, &mut io).await;
        let code = match result {
            Ok(()) => 0,
            Err(e) => {
                if let Some(m) = e.message() {
                    writeln!(io.err, "error: {m}").unwrap();
                }
                e.exit_code()
            }
        };
        Out {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    /// Runs and asserts success, returning stdout.
    pub async fn ok(&mut self, args: &[&str]) -> String {
        let o = self.run(args).await;
        assert_eq!(o.code, 0, "{args:?} failed: {}", o.err);
        o.out
    }
}
