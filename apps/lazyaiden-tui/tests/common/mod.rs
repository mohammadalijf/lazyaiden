#![allow(dead_code)]

use std::io;
use std::path::Path;
use std::sync::Arc;

use fellow_client::FellowApi;
use fellow_client::testing::InMemoryFellow;
use lazyaiden_core::profiles::{FsProfileStore, template};
use lazyaiden_core::{Aiden, Config, MemoryCredentials, Overrides};
use lazyaiden_tui::effects::perform;
use lazyaiden_tui::state::{App, Effect, Event, Msg};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

pub const MORNING: &str = include_str!("../../../../examples/profiles/morning-v60.yaml");

pub type Editor = Box<dyn Fn(&Path) -> io::Result<bool>>;

/// Drives the real reducer and the real effects against an in-memory brewer.
pub struct Tui {
    pub app: App,
    pub aiden: Aiden,
    pub fake: Arc<InMemoryFellow>,
    pub dir: TempDir,
    pub editor: Editor,
    /// Everything put on the clipboard, oldest first.
    pub clipboard: Vec<String>,
    /// When set, clipboard copies fail with this error.
    pub clipboard_error: Option<String>,
}

pub fn config_in(dir: &Path) -> Config {
    Config::resolve(
        &Overrides {
            config_path: Some(dir.join("config.toml")),
            profiles_dir: Some(dir.join("profiles")),
            ..Overrides::default()
        },
        &|_| None,
    )
    .unwrap()
}

impl Tui {
    pub async fn new() -> Self {
        Self::with_fake(InMemoryFellow::new()).await
    }

    pub async fn with_fake(fake: InMemoryFellow) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = config_in(dir.path());
        let fake = Arc::new(fake);
        let store = Arc::new(FsProfileStore::new(&config.profiles_dir));
        let aiden = Aiden::from_parts(fake.clone(), store, config);
        Self::with_aiden(aiden, fake, dir).await
    }

    /// Logged out: local features only.
    pub async fn logged_out() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let aiden = Aiden::connect(config_in(dir.path()), &MemoryCredentials::new()).unwrap();
        Self::with_aiden(aiden, Arc::new(InMemoryFellow::new()), dir).await
    }

    async fn with_aiden(aiden: Aiden, fake: Arc<InMemoryFellow>, dir: TempDir) -> Self {
        let mut t = Self {
            app: App::default(),
            aiden,
            fake,
            dir,
            editor: Box::new(|_| Ok(true)),
            clipboard: Vec::new(),
            clipboard_error: None,
        };
        let pending = t.app.start();
        t.settle(pending).await;
        t
    }

    /// Runs effects and feeds their results back until nothing is pending.
    async fn settle(&mut self, mut pending: Vec<Effect>) {
        let mut guard = 0;
        while !pending.is_empty() {
            guard += 1;
            assert!(guard < 50, "effects never settled");
            let mut next = Vec::new();
            for effect in pending {
                let msg = match effect {
                    Effect::OpenEditor { name } => {
                        let ok = self
                            .aiden
                            .profiles
                            .path_of(&name)
                            .ok()
                            .is_some_and(|p| (self.editor)(&p).unwrap_or(false));
                        Msg::EditorClosed { name, ok }
                    }
                    Effect::Copy { text } => {
                        let error = self.clipboard_error.clone();
                        if error.is_none() {
                            self.clipboard.push(text.clone());
                        }
                        Msg::Copied { text, error }
                    }
                    other => perform(&self.aiden, other).await,
                };
                next.extend(self.app.update(Event::Msg(msg)));
            }
            pending = next;
        }
        assert_eq!(
            self.app.busy, 0,
            "every effect must produce exactly one message"
        );
    }

    pub async fn key(&mut self, code: KeyCode) {
        let pending = self
            .app
            .update(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        self.settle(pending).await;
    }

    pub async fn ctrl(&mut self, c: char) {
        let pending = self.app.update(Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::CONTROL,
        )));
        self.settle(pending).await;
    }

    /// Types each character as a key press.
    pub async fn text(&mut self, text: &str) {
        for c in text.chars() {
            self.key(KeyCode::Char(c)).await;
        }
    }

    /// Whitespace separated tokens: `<Enter>`, `<Esc>`, `<Tab>`, `<BackTab>`, `<Down>`, `<Up>`,
    /// `<Backspace>`, `<Space>`, `<C-s>`, or a literal character sequence.
    pub async fn keys(&mut self, spec: &str) {
        for tok in spec.split_whitespace() {
            match tok {
                "<Enter>" => self.key(KeyCode::Enter).await,
                "<Esc>" => self.key(KeyCode::Esc).await,
                "<Tab>" => self.key(KeyCode::Tab).await,
                "<BackTab>" => self.key(KeyCode::BackTab).await,
                "<Down>" => self.key(KeyCode::Down).await,
                "<Up>" => self.key(KeyCode::Up).await,
                "<Left>" => self.key(KeyCode::Left).await,
                "<Right>" => self.key(KeyCode::Right).await,
                "<Backspace>" => self.key(KeyCode::Backspace).await,
                "<Space>" => self.key(KeyCode::Char(' ')).await,
                "<C-s>" => self.ctrl('s').await,
                "<C-u>" => self.ctrl('u').await,
                "<C-c>" => self.ctrl('c').await,
                literal => self.text(literal).await,
            }
        }
    }

    pub fn message(&self) -> String {
        self.app
            .message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default()
    }

    pub fn titles(&self) -> Vec<String> {
        self.app
            .profiles
            .items
            .iter()
            .map(|r| r.title.clone())
            .collect()
    }

    pub async fn remote_titles(&self) -> Vec<String> {
        self.fake
            .profiles()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.title)
            .collect()
    }

    pub fn write_profile(&self, name: &str, text: &str) {
        let dir = self.dir.path().join("profiles");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.yaml")), text).unwrap();
    }

    pub async fn refresh(&mut self) {
        self.keys("r").await;
    }

    /// Creates a local profile through the service and refreshes.
    pub async fn add_local(&mut self, title: &str) {
        self.aiden.profiles.create_local(template(title)).unwrap();
        self.refresh().await;
    }
}
