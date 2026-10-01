use std::io::{self, Read, Write};
use std::path::Path;

use clap::CommandFactory;
use lazyaiden_core::profiles::{ImportOptions, PullOutcome, PushOutcome, template};
use lazyaiden_core::schedules::{Days, NewSchedule, TimeOfDay};
use lazyaiden_core::{Aiden, Config, CredentialStore, Credentials, Env, Overrides, login, logout};
use serde_json::{Value, json};

use crate::cli::{Cli, Command, DeviceCmd, LoginArgs, ProfileCmd, ScheduleCmd};
use crate::error::{CliError, device_name, draft_json};
use crate::render::{pull_label, push_label, state_label, table};

type Result<T = (), E = CliError> = std::result::Result<T, E>;

/// Streams a command talks to; injectable for tests.
pub struct Io<'a> {
    /// Normal output.
    pub out: &'a mut dyn Write,
    /// Diagnostics and prompts.
    pub err: &'a mut dyn Write,
    /// Input for `-i -` and `--password-stdin`.
    pub stdin: &'a mut dyn Read,
    /// Opens a file for interactive editing; returns whether the editor succeeded.
    pub editor: &'a dyn Fn(&Path) -> io::Result<bool>,
}

/// Runs `$VISUAL`, `$EDITOR` or `vi` on `path`, attached to the terminal.
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

fn emit(io: &mut Io<'_>, json: bool, human: String, value: Value) -> Result {
    if json {
        writeln!(
            io.out,
            "{}",
            serde_json::to_string_pretty(&value).expect("json")
        )?;
    } else {
        write!(io.out, "{human}")?;
    }
    Ok(())
}

/// Prints the top-level manual page, or writes every page into `dir`.
fn man(dir: Option<&Path>, io: &mut Io<'_>) -> Result {
    match dir {
        None => clap_mangen::Man::new(Cli::command()).render(io.out)?,
        Some(dir) => {
            std::fs::create_dir_all(dir)?;
            clap_mangen::generate_to(Cli::command(), dir)?;
            writeln!(io.out, "Wrote manual pages to {}", dir.display())?;
        }
    }
    Ok(())
}

/// Entry point used by `main`: resolves configuration, then dispatches.
///
/// `env` supplies environment variables and `creds` the credential store, so
/// tests can run the full flow hermetically.
pub async fn execute(
    cli: Cli,
    io: &mut Io<'_>,
    env: Env<'_>,
    creds: &dyn CredentialStore,
) -> Result {
    if let Command::Completions { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "lazyaiden-cli", io.out);
        return Ok(());
    }
    if let Command::Man { dir } = &cli.command {
        return man(dir.as_deref(), io);
    }
    let overrides = Overrides {
        config_path: cli.config.clone(),
        profiles_dir: cli.profiles_dir.clone(),
        brewer_id: cli.brewer.clone(),
        base_url: cli.base_url.clone(),
    };
    let config = Config::resolve(&overrides, env)?;
    match &cli.command {
        Command::Login(args) => return login_cmd(args, &config, creds, io, env).await,
        Command::Logout => {
            logout(creds)?;
            return emit(
                io,
                cli.json,
                "Logged out.\n".into(),
                json!({"loggedOut": true}),
            );
        }
        _ => {}
    }
    let aiden = if cli.demo {
        Aiden::demo(overrides.explicit_profiles_dir(env)).await?
    } else {
        Aiden::connect(config, creds)?
    };
    run(&cli.command, cli.json, &aiden, io).await
}

async fn login_cmd(
    args: &LoginArgs,
    config: &Config,
    creds: &dyn CredentialStore,
    io: &mut Io<'_>,
    env: Env<'_>,
) -> Result {
    let read_line = |io: &mut Io<'_>| -> Result<String> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        while io.stdin.read(&mut byte)? == 1 && byte[0] != b'\n' {
            line.push(byte[0]);
        }
        Ok(String::from_utf8_lossy(&line)
            .trim_end_matches('\r')
            .to_owned())
    };
    let email = match args.email.clone().or_else(|| env("FELLOW_EMAIL")) {
        Some(e) => e,
        None if args.password_stdin => {
            return Err(CliError::Usage(
                "--password-stdin needs --email (or FELLOW_EMAIL)".into(),
            ));
        }
        None => {
            write!(io.err, "Email: ")?;
            io.err.flush()?;
            read_line(io)?
        }
    };
    let password = if args.password_stdin {
        read_line(io)?
    } else {
        rpassword::prompt_password("Password: ")?
    };
    if email.is_empty() || password.is_empty() {
        return Err(CliError::Usage(
            "email and password must not be empty".into(),
        ));
    }
    login(config, Credentials::new(&email, password), creds).await?;
    writeln!(io.out, "Logged in as {email}; credentials saved.")?;
    Ok(())
}

/// Executes a (non-login) command against an already wired [`Aiden`].
pub async fn run(cmd: &Command, json: bool, aiden: &Aiden, io: &mut Io<'_>) -> Result {
    match cmd {
        Command::Device(c) => device(c, json, aiden, io).await,
        Command::Profile(c) => profile(c, json, aiden, io).await,
        Command::Schedule(c) => schedule(c, json, aiden, io).await,
        Command::Login(_) | Command::Logout | Command::Completions { .. } | Command::Man { .. } => {
            Err(CliError::Usage(
                "this command is handled before an Aiden session exists".into(),
            ))
        }
    }
}

// ---- device -----------------------------------------------------------------

async fn device(cmd: &DeviceCmd, json: bool, aiden: &Aiden, io: &mut Io<'_>) -> Result {
    match cmd {
        DeviceCmd::List => {
            let devices = aiden.devices.list().await?;
            let rows: Vec<Vec<String>> = devices
                .iter()
                .map(|v| {
                    vec![
                        if v.active { "*" } else { "" }.to_owned(),
                        v.device.id.clone(),
                        device_name(&v.device),
                    ]
                })
                .collect();
            let value = json!({"devices": devices.iter().map(|v| json!({
                "id": v.device.id, "name": v.device.display_name, "active": v.active
            })).collect::<Vec<_>>()});
            emit(io, json, table(&["", "ID", "NAME"], &rows), value)
        }
        DeviceCmd::Use { brewer } => {
            let d = aiden.use_brewer(brewer).await?;
            emit(
                io,
                json,
                format!("Using brewer {} ({}).\n", device_name(&d), d.id),
                json!({"id": d.id, "name": d.display_name}),
            )
        }
        DeviceCmd::Info => {
            let d = aiden.devices.active().await?;
            let mut human = format!("id:   {}\nname: {}\n", d.id, device_name(&d));
            for (k, v) in &d.extra {
                if !v.is_object() && !v.is_array() {
                    human.push_str(&format!("{k}: {v}\n"));
                }
            }
            emit(
                io,
                json,
                human,
                serde_json::to_value(&d).expect("device json"),
            )
        }
    }
}

// ---- profile ------------------------------------------------------------------

fn push_json(name: &str, r: &lazyaiden_core::profiles::Result<PushOutcome>) -> Value {
    match r {
        Ok(o) => json!({"name": name, "action": push_label(o.action), "remoteId": o.remote_id}),
        Err(e) => json!({"name": name, "error": e.to_string()}),
    }
}

fn pull_json(name: &str, r: &lazyaiden_core::profiles::Result<PullOutcome>) -> Value {
    match r {
        Ok(o) => json!({"name": o.name, "action": pull_label(o.action), "remoteId": o.remote_id}),
        Err(e) => json!({"title": name, "error": e.to_string()}),
    }
}

async fn profile(cmd: &ProfileCmd, json: bool, aiden: &Aiden, io: &mut Io<'_>) -> Result {
    let svc = &aiden.profiles;
    match cmd {
        ProfileCmd::List { local: true } => {
            let listing = svc.list_local()?;
            let rows: Vec<Vec<String>> = listing
                .profiles
                .iter()
                .map(|p| vec![p.name.clone(), p.draft.title.clone()])
                .collect();
            for p in &listing.problems {
                writeln!(io.err, "warning: {}: {}", p.name, p.message)?;
            }
            let value = json!({
                "profiles": listing.profiles.iter().map(|p| json!({"name": p.name, "title": p.draft.title})).collect::<Vec<_>>(),
                "problems": listing.problems.iter().map(|p| json!({"name": p.name, "message": p.message})).collect::<Vec<_>>(),
            });
            emit(io, json, table(&["NAME", "TITLE"], &rows), value)
        }
        ProfileCmd::List { local: false } => {
            let report = svc.status().await?;
            let rows: Vec<Vec<String>> = report
                .entries
                .iter()
                .map(|e| {
                    vec![
                        e.name.clone().unwrap_or_else(|| "-".into()),
                        e.title.clone(),
                        state_label(e.state).to_owned(),
                        e.remote_id.clone().unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect();
            for p in &report.problems {
                writeln!(io.err, "warning: {}: {}", p.name, p.message)?;
            }
            let value = json!({
                "profiles": report.entries.iter().map(|e| json!({
                    "name": e.name, "title": e.title, "state": state_label(e.state), "remoteId": e.remote_id
                })).collect::<Vec<_>>(),
                "problems": report.problems.iter().map(|p| json!({"name": p.name, "message": p.message})).collect::<Vec<_>>(),
            });
            emit(
                io,
                json,
                table(&["NAME", "TITLE", "STATE", "REMOTE"], &rows),
                value,
            )
        }
        ProfileCmd::Show { profile } => {
            let p = svc.get_local(profile)?;
            emit(io, json, svc.export_str(&p.name)?, draft_json(&p.draft))
        }
        ProfileCmd::Add {
            input,
            name,
            overwrite,
        } => {
            let options = ImportOptions {
                name: name.clone(),
                overwrite: *overwrite,
            };
            let p = if input == "-" {
                let mut text = String::new();
                io.stdin.read_to_string(&mut text)?;
                svc.import_str(&text, &options)?
            } else {
                svc.import_file(Path::new(input), &options)?
            };
            emit(
                io,
                json,
                format!("Imported {:?} as {}.\n", p.draft.title, p.name),
                json!({"name": p.name, "title": p.draft.title}),
            )
        }
        ProfileCmd::New { title } => {
            let p = svc.create_local(template(title))?;
            let path = svc.path_of(&p.name)?;
            emit(
                io,
                json,
                format!("Created {} ({}).\n", p.name, path.display()),
                json!({"name": p.name, "title": p.draft.title, "path": path}),
            )
        }
        ProfileCmd::Export { profile, output } => match output.as_deref() {
            None | Some("-") => {
                let text = svc.export_str(profile)?;
                write!(io.out, "{text}")?;
                Ok(())
            }
            Some(path) => {
                svc.export_file(profile, Path::new(path))?;
                emit(io, json, format!("Wrote {path}.\n"), json!({"path": path}))
            }
        },
        ProfileCmd::Edit { profile } => {
            let local = svc.get_local(profile)?;
            let path = svc.path_of(&local.name)?;
            if !(io.editor)(&path)? {
                return Err(CliError::Io(io::Error::other(
                    "the editor exited with an error",
                )));
            }
            match svc.validate_local(&local.name) {
                Ok(p) => emit(
                    io,
                    json,
                    format!("{} is valid.\n", p.name),
                    json!({"name": p.name, "valid": true}),
                ),
                Err(e) => {
                    writeln!(
                        io.err,
                        "{} was saved but is not usable yet; run `edit` again to fix it.",
                        local.name
                    )?;
                    Err(e.into())
                }
            }
        }
        ProfileCmd::Rm {
            profile, remote, ..
        } => {
            let local = svc.get_local(profile)?;
            if *remote {
                svc.delete_remote(&local.name).await?;
            }
            svc.delete_local(&local.name)?;
            emit(
                io,
                json,
                format!(
                    "Deleted {}{}.\n",
                    local.name,
                    if *remote { " (local and remote)" } else { "" }
                ),
                json!({"name": local.name, "remote": remote}),
            )
        }
        ProfileCmd::Push { profile, all } => {
            let results: Vec<(String, _)> = match (profile, all) {
                (Some(p), false) => {
                    let name = svc
                        .get_local(p)
                        .map(|l| l.name)
                        .unwrap_or_else(|_| p.clone());
                    vec![(name, Ok(svc.push(p).await?))]
                }
                (None, true) => svc.push_all().await?,
                _ => return Err(CliError::Usage("give a profile or --all".into())),
            };
            report(io, json, &results, push_json, |n, o: &PushOutcome| {
                format!("{:<10} {n} -> {}", push_label(o.action), o.remote_id)
            })
        }
        ProfileCmd::Pull { profile, all } => {
            let results: Vec<(String, _)> = match (profile, all) {
                (Some(p), false) => vec![(p.clone(), Ok(svc.pull(p).await?))],
                (None, true) => svc.pull_all().await?,
                _ => return Err(CliError::Usage("give a profile or --all".into())),
            };
            report(io, json, &results, pull_json, |_, o: &PullOutcome| {
                format!("{:<10} {} <- {}", pull_label(o.action), o.name, o.remote_id)
            })
        }
        ProfileCmd::Diff { profile } => {
            let changes = svc.diff(profile).await?;
            let human = if changes.is_empty() {
                "No differences.\n".to_owned()
            } else {
                changes
                    .iter()
                    .map(|c| format!("{}: local {} / remote {}\n", c.field, c.local, c.remote))
                    .collect()
            };
            let value = json!({"changes": changes.iter().map(|c| json!({
                "field": c.field, "local": c.local, "remote": c.remote
            })).collect::<Vec<_>>()});
            emit(io, json, human, value)
        }
        ProfileCmd::Share { profile } => {
            let link = svc.share_link(profile).await?;
            emit(io, json, format!("{link}\n"), json!({"link": link}))
        }
        ProfileCmd::ImportLink { link } => {
            let p = svc.import_from_link(link).await?;
            emit(
                io,
                json,
                format!("Imported {:?} as {} (not pushed).\n", p.draft.title, p.name),
                json!({"name": p.name, "title": p.draft.title}),
            )
        }
    }
}

/// Prints per-item results; fails with [`CliError::Reported`] when any item failed.
fn report<T>(
    io: &mut Io<'_>,
    json: bool,
    results: &[(String, lazyaiden_core::profiles::Result<T>)],
    to_json: impl Fn(&str, &lazyaiden_core::profiles::Result<T>) -> Value,
    ok_line: impl Fn(&str, &T) -> String,
) -> Result {
    let failed = results.iter().any(|(_, r)| r.is_err());
    if json {
        let items: Vec<Value> = results.iter().map(|(n, r)| to_json(n, r)).collect();
        emit(io, true, String::new(), json!({"results": items}))?;
    } else {
        for (name, r) in results {
            match r {
                Ok(o) => writeln!(io.out, "{}", ok_line(name, o))?,
                Err(e) => writeln!(io.out, "{:<10} {name}: {e}", "error")?,
            }
        }
        if results.is_empty() {
            writeln!(io.out, "Nothing to do.")?;
        }
    }
    if failed {
        Err(CliError::Reported)
    } else {
        Ok(())
    }
}

// ---- schedule -------------------------------------------------------------------

async fn schedule(cmd: &ScheduleCmd, json: bool, aiden: &Aiden, io: &mut Io<'_>) -> Result {
    let svc = &aiden.schedules;
    match cmd {
        ScheduleCmd::List => {
            let views = svc.list().await?;
            let rows: Vec<Vec<String>> = views
                .iter()
                .map(|v| {
                    let d = &v.schedule.draft;
                    vec![
                        v.schedule.id.clone(),
                        if d.enabled { "yes" } else { "no" }.into(),
                        v.days().to_string(),
                        v.time().map_or_else(
                            || d.second_from_start_of_the_day.to_string(),
                            |t| t.to_string(),
                        ),
                        format!("{} ml", d.amount_of_water),
                        match &v.profile_title {
                            Some(t) => format!("{t} ({})", d.profile_id),
                            None => format!("(missing) {}", d.profile_id),
                        },
                    ]
                })
                .collect();
            let value = json!({"schedules": views.iter().map(|v| {
                let d = &v.schedule.draft;
                json!({
                    "id": v.schedule.id, "enabled": d.enabled, "days": v.days().to_string(),
                    "time": v.time().map(|t| t.to_string()), "waterMl": d.amount_of_water,
                    "profileId": d.profile_id, "profileTitle": v.profile_title,
                })
            }).collect::<Vec<_>>()});
            emit(
                io,
                json,
                table(&["ID", "ON", "DAYS", "TIME", "WATER", "PROFILE"], &rows),
                value,
            )
        }
        ScheduleCmd::Add {
            profile,
            days,
            time,
            water,
            disabled,
        } => {
            let days: Days = days.parse()?;
            let time: TimeOfDay = time.parse()?;
            let s = svc
                .create(NewSchedule {
                    days,
                    time,
                    water_ml: *water,
                    profile: profile.clone(),
                    enabled: !disabled,
                })
                .await?;
            emit(
                io,
                json,
                format!(
                    "Created schedule {} ({days} at {time}, {water} ml).\n",
                    s.id
                ),
                json!({"id": s.id, "days": days.to_string(), "time": time.to_string(), "waterMl": water, "enabled": s.draft.enabled}),
            )
        }
        ScheduleCmd::Toggle { id, on, off } => {
            let enabled = if *on || *off {
                svc.set_enabled(id, *on).await?;
                *on
            } else {
                svc.toggle(id).await?
            };
            emit(
                io,
                json,
                format!(
                    "Schedule {id} is now {}.\n",
                    if enabled { "enabled" } else { "disabled" }
                ),
                json!({"id": id, "enabled": enabled}),
            )
        }
        ScheduleCmd::Rm { id } => {
            svc.delete(id).await?;
            emit(
                io,
                json,
                format!("Deleted schedule {id}.\n"),
                json!({"id": id}),
            )
        }
    }
}
