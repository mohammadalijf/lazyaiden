use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use lazyaiden_core::{Aiden, Config, LayeredCredentials, Overrides, process_env};
use lazyaiden_tui::Args;
use lazyaiden_tui::runtime::{default_clipboard, default_editor, run};

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
