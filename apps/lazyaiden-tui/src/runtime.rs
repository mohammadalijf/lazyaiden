//! Terminal ownership and the event loop.

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use lazyaiden_core::Aiden;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyEventKind};
use tokio::sync::mpsc;

use crate::effects::perform;
use crate::state::{App, Effect, Event, Msg};
use crate::ui::draw;

/// Opens `path` in the user's editor; returns whether it exited successfully.
pub type Editor = dyn Fn(&Path) -> io::Result<bool> + Send + Sync;

/// `$VISUAL`, `$EDITOR` or `vi`.
pub fn default_editor(path: &Path) -> io::Result<bool> {
    let spec = std::env::var("VISUAL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("EDITOR").ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "vi".into());
    let mut parts = spec.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    Ok(std::process::Command::new(program)
        .args(parts)
        .arg(path)
        .status()?
        .success())
}

/// Puts text on the clipboard; the error explains a failure.
pub type Clipboard = dyn Fn(&str) -> Result<(), String> + Send + Sync;

/// The system clipboard, falling back to the terminal's OSC 52 sequence (which
/// also works over SSH) when there is no system clipboard to talk to.
pub fn default_clipboard(text: &str) -> Result<(), String> {
    use std::sync::{Mutex, OnceLock};
    // Some platforms drop the clipboard contents with the last handle; keep one alive.
    static SYSTEM: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
    let system = SYSTEM.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()));
    let copied = system
        .lock()
        .ok()
        .and_then(|mut c| c.as_mut().map(|c| c.set_text(text).is_ok()))
        .unwrap_or(false);
    if copied {
        return Ok(());
    }
    use base64::Engine as _;
    use std::io::Write as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut out = io::stdout();
    write!(out, "\x1b]52;c;{encoded}\x07")
        .and_then(|()| out.flush())
        .map_err(|e| e.to_string())
}

/// Runs the interface until the user quits. Restores the terminal on exit,
/// including after a panic (ratatui installs the panic hook).
pub async fn run(
    aiden: Arc<Aiden>,
    editor: Arc<Editor>,
    clipboard: Arc<Clipboard>,
) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, aiden, editor, clipboard).await;
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut DefaultTerminal,
    aiden: Arc<Aiden>,
    editor: Arc<Editor>,
    clipboard: Arc<Clipboard>,
) -> io::Result<()> {
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Msg>();
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<TermEvent>();

    // Terminal input is blocking; read it on its own thread.
    let reader = std::thread::spawn({
        let input_tx = input_tx.clone();
        move || {
            while !input_tx.is_closed() {
                if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    continue;
                }
                // Keys drive the app; a resize only needs a redraw.
                let forward = match event::read() {
                    Ok(TermEvent::Key(k)) if k.kind != KeyEventKind::Release => TermEvent::Key(k),
                    Ok(resize @ TermEvent::Resize(..)) => resize,
                    _ => continue,
                };
                if input_tx.send(forward).is_err() {
                    break;
                }
            }
        }
    });

    // Drives the spinner while effects are in flight.
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut app = App::default();
    let mut pending = app.start();
    loop {
        for effect in std::mem::take(&mut pending) {
            match effect {
                Effect::OpenEditor { name } => {
                    // Leave the alternate screen while the editor owns the terminal.
                    let path = aiden.profiles.path_of(&name);
                    ratatui::restore();
                    let ok = path.is_ok_and(|p| editor(&p).unwrap_or(false));
                    *terminal = ratatui::init();
                    let _ = msg_tx.send(Msg::EditorClosed { name, ok });
                }
                Effect::Copy { text } => {
                    let error = clipboard(&text).err();
                    let _ = msg_tx.send(Msg::Copied { text, error });
                }
                effect => {
                    let (aiden, tx) = (aiden.clone(), msg_tx.clone());
                    tokio::spawn(async move {
                        let _ = tx.send(perform(&aiden, effect).await);
                    });
                }
            }
        }
        terminal.draw(|f| draw(f, &app))?;
        if app.quit {
            break;
        }
        pending = tokio::select! {
            Some(input) = input_rx.recv() => match input {
                TermEvent::Key(k) => app.update(Event::Key(k)),
                // The next draw picks up the new size.
                _ => Vec::new(),
            },
            Some(m) = msg_rx.recv() => app.update(Event::Msg(m)),
            _ = ticker.tick(), if app.busy > 0 => app.update(Event::Tick),
            else => break,
        };
    }
    drop(input_rx);
    let _ = reader.join();
    Ok(())
}
