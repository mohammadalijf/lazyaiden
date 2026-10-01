use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

/// Manage your Fellow Aiden brewer: profiles, schedules and devices.
///
/// Profiles live as YAML files on disk; `push` and `pull` sync them with the brewer.
/// Credentials come from FELLOW_EMAIL / FELLOW_PASSWORD or the OS keychain (`login`).
#[derive(Debug, Parser)]
#[command(name = "lazyaiden-cli", version, propagate_version = true)]
pub struct Cli {
    /// Config file (default: $XDG_CONFIG_HOME/lazyaiden/config.toml or ~/.config/lazyaiden/config.toml).
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Directory with local profile YAML files [env: LAZYAIDEN_PROFILES_DIR].
    #[arg(long, global = true, value_name = "DIR")]
    pub profiles_dir: Option<PathBuf>,
    /// Brewer id to act on [env: LAZYAIDEN_BREWER_ID].
    #[arg(long, global = true, value_name = "ID")]
    pub brewer: Option<String>,
    /// Fellow API base URL (testing and proxies) [env: LAZYAIDEN_BASE_URL].
    #[arg(long, global = true, value_name = "URL", hide = true)]
    pub base_url: Option<String>,
    /// Print machine-readable JSON instead of text.
    #[arg(long, global = true)]
    pub json: bool,
    /// Use an offline in-memory brewer with sample data (state is lost on exit).
    #[arg(long, global = true)]
    pub demo: bool,
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Store your Fellow credentials in the OS keychain (verified first).
    Login(LoginArgs),
    /// Remove stored credentials.
    Logout,
    /// List brewers, choose the active one, show its details.
    #[command(subcommand)]
    Device(DeviceCmd),
    /// Create, edit, import, export and sync brew profiles.
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// Manage recurring brew schedules on the brewer.
    #[command(subcommand)]
    Schedule(ScheduleCmd),
    /// Print a shell completion script.
    Completions {
        /// Target shell.
        shell: Shell,
    },
    /// Print the manual page, or install a page for every command with --dir.
    ///
    /// `lazyaiden-cli man --dir ~/.cargo/share/man/man1` makes `man lazyaiden-cli` work
    /// for a `cargo install`ed binary without further setup.
    Man {
        /// Write `lazyaiden-cli.1` and one page per subcommand into DIR (created if missing).
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
    },
}

/// Arguments of `login`.
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Account email (prompted when omitted).
    #[arg(long)]
    pub email: Option<String>,
    /// Read the password from the first line of stdin instead of prompting.
    #[arg(long)]
    pub password_stdin: bool,
}

/// `device` subcommands.
#[derive(Debug, Subcommand)]
pub enum DeviceCmd {
    /// List brewers on the account.
    List,
    /// Make a brewer the active one and remember it in the config file.
    Use {
        /// Brewer id or display name.
        brewer: String,
    },
    /// Show the active brewer.
    Info,
}

/// `profile` subcommands.
#[derive(Debug, Subcommand)]
pub enum ProfileCmd {
    /// List profiles with their sync state (needs login; `--local` works offline).
    List {
        /// Only list the local files; no network.
        #[arg(long)]
        local: bool,
    },
    /// Print a local profile as YAML.
    Show {
        /// File name or title.
        profile: String,
    },
    /// Import a profile from a YAML or JSON file (`-` reads stdin).
    Add {
        /// File to import.
        #[arg(short, long, value_name = "FILE")]
        input: String,
        /// Local name to use instead of one derived from the title.
        #[arg(long)]
        name: Option<String>,
        /// Replace an existing profile with the same name or title.
        #[arg(long)]
        overwrite: bool,
    },
    /// Create a new local profile from a template.
    New {
        /// Title of the new profile.
        title: String,
    },
    /// Write a local profile as YAML (stdout by default).
    Export {
        /// File name or title.
        profile: String,
        /// Output file (`-` for stdout).
        #[arg(short, long, value_name = "FILE")]
        output: Option<String>,
    },
    /// Open a local profile in $VISUAL/$EDITOR and validate it afterwards.
    Edit {
        /// File name or title.
        profile: String,
    },
    /// Delete a local profile (and with --remote also its copy on the brewer).
    Rm {
        /// File name or title.
        profile: String,
        /// Also delete the profile on the brewer.
        #[arg(long, requires = "yes")]
        remote: bool,
        /// Confirm deleting on the brewer.
        #[arg(long)]
        yes: bool,
    },
    /// Send local profiles to the brewer (creates or updates; never duplicates).
    Push {
        /// File name or title.
        profile: Option<String>,
        /// Push every local profile.
        #[arg(long, conflicts_with = "profile")]
        all: bool,
    },
    /// Fetch profiles from the brewer into local files.
    Pull {
        /// Remote id or title.
        profile: Option<String>,
        /// Pull every remote profile.
        #[arg(long, conflicts_with = "profile")]
        all: bool,
    },
    /// Show which fields differ between a local profile and its remote copy.
    Diff {
        /// File name or title.
        profile: String,
    },
    /// Create a share link for a pushed profile.
    Share {
        /// File name or title.
        profile: String,
    },
    /// Import a profile from a share link (kept local until you push).
    ImportLink {
        /// Share link or brew id.
        link: String,
    },
}

/// `schedule` subcommands.
#[derive(Debug, Subcommand)]
pub enum ScheduleCmd {
    /// List schedules.
    List,
    /// Create a schedule.
    Add {
        /// Profile id (p12) or title.
        #[arg(long)]
        profile: String,
        /// Weekdays: `mon,wed,fri`, `mon-fri`, `weekdays`, `weekends`, `daily`.
        #[arg(long)]
        days: String,
        /// Start time: `07:30`, `7:30am`.
        #[arg(long)]
        time: String,
        /// Water in millilitres (150-1500).
        #[arg(long, value_name = "ML")]
        water: u32,
        /// Create the schedule disabled.
        #[arg(long)]
        disabled: bool,
    },
    /// Enable/disable a schedule (flips it when neither flag is given).
    Toggle {
        /// Schedule id.
        id: String,
        /// Force enabled.
        #[arg(long, conflicts_with = "off")]
        on: bool,
        /// Force disabled.
        #[arg(long)]
        off: bool,
    },
    /// Delete a schedule.
    Rm {
        /// Schedule id.
        id: String,
    },
}
