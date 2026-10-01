//! Workspace tasks, run with `cargo xtask <task>`.
//!
//! * `dist-assets <DIR>`: render the man pages and shell completions of both
//!   apps into `DIR/man` and `DIR/completions`. They are platform independent,
//!   so the release workflow renders them once and ships them in every archive.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs, io};

use clap::CommandFactory;
use clap_complete::Shell;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let result = match (args.next().as_deref(), args.next(), args.next()) {
        (Some("dist-assets"), Some(dir), None) => dist_assets(Path::new(&dir)),
        _ => {
            eprintln!("usage: cargo xtask dist-assets <DIR>");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dist_assets(dir: &Path) -> io::Result<()> {
    let man = dir.join("man");
    let completions = dir.join("completions");
    fs::create_dir_all(&man)?;
    fs::create_dir_all(&completions)?;
    for cmd in [
        lazyaiden_tui::Args::command(),
        lazyaiden_cli::Cli::command(),
    ] {
        let name = cmd.get_name().to_owned();
        // One page for the command and one per subcommand (`lazyaiden-cli-profile-push.1`, ...).
        clap_mangen::generate_to(cmd.clone(), &man)?;
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let path = clap_complete::generate_to(shell, &mut cmd.clone(), &name, &completions)?;
            println!("{}", rel(dir, path).display());
        }
    }
    for entry in fs::read_dir(&man)? {
        println!("{}", rel(dir, entry?.path()).display());
    }
    Ok(())
}

fn rel(base: &Path, path: PathBuf) -> PathBuf {
    path.strip_prefix(base)
        .map(Path::to_path_buf)
        .unwrap_or(path)
}
