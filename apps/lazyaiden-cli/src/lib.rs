//! `lazyaiden-cli`: the headless front end of Aiden.
//!
//! Everything lives in this library so it can be tested without spawning a
//! process: [`Cli`] is the clap definition, [`execute`] wires configuration and
//! credentials into an [`lazyaiden_core::Aiden`], and [`run`] performs a command
//! against any `Aiden`, writing to the supplied [`Io`].

#![warn(missing_docs)]

mod app;
mod cli;
mod error;
mod render;

pub use app::{Io, default_editor, execute, run};
pub use cli::{Cli, Command, DeviceCmd, LoginArgs, ProfileCmd, ScheduleCmd};
pub use error::CliError;
