use std::io;
use std::process::ExitCode;

use clap::Parser;
use lazyaiden_cli::{Cli, Io, default_editor, execute};
use lazyaiden_core::{LayeredCredentials, process_env};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let creds = LayeredCredentials::standard();
    let (mut stdout, mut stderr, mut stdin) =
        (io::stdout().lock(), io::stderr().lock(), io::stdin().lock());
    let mut io = Io {
        out: &mut stdout,
        err: &mut stderr,
        stdin: &mut stdin,
        editor: &default_editor,
    };
    let result = execute(cli, &mut io, &process_env, &creds).await;
    let _ = io.out.flush();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if let Some(msg) = e.message() {
                let _ = writeln!(io.err, "error: {msg}");
            }
            ExitCode::from(e.exit_code())
        }
    }
}
