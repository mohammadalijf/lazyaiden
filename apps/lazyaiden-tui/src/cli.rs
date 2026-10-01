//! Command-line arguments of the `lazyaiden` binary.
//!
//! Kept in the library so release tooling can render the man page and shell
//! completions from the same definition the binary parses.

use std::path::PathBuf;

use clap::Parser;

/// Terminal UI for your Fellow Aiden brewer.
///
/// Credentials come from FELLOW_EMAIL / FELLOW_PASSWORD or the OS keychain
/// (store them once with `lazyaiden-cli login`).
#[derive(Debug, Parser)]
#[command(name = "lazyaiden", version)]
pub struct Args {
    /// Config file.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Directory with local profile YAML files [env: LAZYAIDEN_PROFILES_DIR].
    #[arg(long, value_name = "DIR")]
    pub profiles_dir: Option<PathBuf>,
    /// Brewer id to act on [env: LAZYAIDEN_BREWER_ID].
    #[arg(long, value_name = "ID")]
    pub brewer: Option<String>,
    /// Use an offline in-memory brewer with sample data (state is lost on exit).
    #[arg(long)]
    pub demo: bool,
}
