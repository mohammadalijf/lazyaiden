use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use lazyaiden_core::{Aiden, Config, LayeredCredentials, Overrides, process_env};
use lazyaiden_tui::runtime::{default_clipboard, default_editor, run};

/// Terminal UI for your Fellow Aiden brewer.
///
/// Credentials come from FELLOW_EMAIL / FELLOW_PASSWORD or the OS keychain
/// (store them once with `lazyaiden-cli login`).
#[derive(Debug, Parser)]
#[command(name = "lazyaiden", version)]
struct Args {
    /// Config file.
    #[arg(long, value_name = "FILE")]
    config: Option<std::path::PathBuf>,
    /// Directory with local profile YAML files [env: LAZYAIDEN_PROFILES_DIR].
    #[arg(long, value_name = "DIR")]
    profiles_dir: Option<std::path::PathBuf>,
    /// Brewer id to act on [env: LAZYAIDEN_BREWER_ID].
    #[arg(long, value_name = "ID")]
    brewer: Option<String>,
    /// Use an offline in-memory brewer with sample data (state is lost on exit).
    #[arg(long)]
    demo: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let overrides = Overrides {
        config_path: args.config,
        profiles_dir: args.profiles_dir,
        brewer_id: args.brewer,
        base_url: None,
    };
    let config = match Config::resolve(&overrides, &process_env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let aiden = if args.demo {
        match Aiden::demo(overrides.explicit_profiles_dir(&process_env)).await {
            Ok(a) => a,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        match Aiden::connect(config, &LayeredCredentials::standard()) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        }
    };
    match run(
        Arc::new(aiden),
        Arc::new(default_editor),
        Arc::new(default_clipboard),
    )
    .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
