use std::{env, error::Error, io::BufRead, net::SocketAddr, path::PathBuf, process::ExitCode};

use be_server::{ServerConfig, backup};
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  be-server [--addr ADDR] [--data-dir DIR] [--allow-registration]
  be-server [--data-dir DIR] --add-account EMAIL NAME WORKSPACE < password
  be-server backup-key > KEY-FILE
  be-server backup [--data-dir DIR] --to TARGET --key-file KEY-FILE
  be-server restore --from TARGET --key-file KEY-FILE --data-dir EMPTY-DIR [--snapshot NAME]
  be-server verify [--data-dir DIR]

  --addr ADDR               Address to listen on (default: 127.0.0.1:9090)
  --data-dir DIR            Where accounts and blocks are stored (default: be-server-data)
  --allow-registration      Let anyone who reaches the server make an account; without it,
                            accounts are made with --add-account
  --add-account             Make an account that owns a new workspace, reading its password
                            from the first line of standard input

A backup uploads every object the database references that the target does not
have yet, never deleting one, and then the database, sealed with the backup
key. It keeps every database from the last 48 hours, one a day for 30 days and
one a month for a year. TARGET is a directory, or bunny:ZONE for a Bunny
Storage zone, with the zone's password in BUNNY_STORAGE_KEY and its region's
endpoint in BUNNY_STORAGE_ENDPOINT (default: https://storage.bunnycdn.com).
Without the backup key a backup cannot be restored, and without the recovery
phrase its notes cannot be read.

RUST_LOG sets what is logged (default: info).";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1).peekable();
    match arguments.peek().map(String::as_str) {
        Some("backup-key") => {
            println!("{}", backup::BackupKey::generate().to_hex());
            return Ok(());
        }
        Some(command @ ("backup" | "restore" | "verify")) => {
            let command = command.to_owned();
            arguments.next();
            let options = Options::parse(arguments).map_err(|error| error.to_string())?;
            return tokio::task::spawn_blocking(move || maintain(&command, options))
                .await?
                .map_err(|error| error.to_string().into());
        }
        _ => {}
    }
    let mut address: SocketAddr = "127.0.0.1:9090".parse()?;
    let mut data_dir = PathBuf::from("be-server-data");
    let mut config = ServerConfig::default();
    let mut account: Option<(String, String, String)> = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--addr" | "--address" => {
                address = arguments.next().ok_or("--addr needs a value")?.parse()?;
            }
            "--data-dir" => {
                data_dir = PathBuf::from(arguments.next().ok_or("--data-dir needs a value")?);
            }
            "--allow-registration" => config.allow_registration = true,
            "--add-account" => {
                let email = arguments.next().ok_or("--add-account needs an email")?;
                let display_name = arguments.next().ok_or("--add-account needs a name")?;
                let workspace = arguments.next().ok_or("--add-account needs a workspace")?;
                account = Some((email, display_name, workspace));
            }
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }

    if let Some((email, display_name, workspace)) = account {
        eprintln!("Password for {email}:");
        let mut password = String::new();
        std::io::stdin().lock().read_line(&mut password)?;
        let password = password.trim_end_matches(['\r', '\n']);
        let (account, workspace) =
            be_server::add_account(&data_dir, &email, &display_name, password, &workspace).await?;
        println!("account {account} owns workspace {workspace}");
        return Ok(());
    }

    let listener = TcpListener::bind(address).await?;
    tracing::info!(
        address = %listener.local_addr()?,
        data = %data_dir.display(),
        registration = config.allow_registration,
        "be-server listening"
    );
    be_server::serve_with_config(listener, data_dir, config, shutdown()).await?;
    tracing::info!("be-server stopped");
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = terminate.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[derive(Default)]
struct Options {
    data_dir: Option<PathBuf>,
    target: Option<String>,
    key_file: Option<PathBuf>,
    snapshot: Option<String>,
}

impl Options {
    fn parse(
        mut arguments: impl Iterator<Item = String>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut options = Self::default();
        while let Some(argument) = arguments.next() {
            let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
            match argument.as_str() {
                "--data-dir" => options.data_dir = Some(PathBuf::from(value()?)),
                "--to" | "--from" => options.target = Some(value()?),
                "--key-file" => options.key_file = Some(PathBuf::from(value()?)),
                "--snapshot" => options.snapshot = Some(value()?),
                other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
            }
        }
        Ok(options)
    }

    fn data_dir(&self) -> PathBuf {
        self.data_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("be-server-data"))
    }

    fn target(&self) -> Result<Box<dyn backup::Target>, Box<dyn Error + Send + Sync>> {
        let target = self.target.as_deref().ok_or("name a backup target")?;
        Ok(match target.strip_prefix("bunny:") {
            Some(zone) => Box::new(backup::Bunny::new(
                &env::var("BUNNY_STORAGE_ENDPOINT")
                    .unwrap_or_else(|_| "https://storage.bunnycdn.com".to_owned()),
                zone,
                env::var("BUNNY_STORAGE_KEY").map_err(|_| "set BUNNY_STORAGE_KEY")?,
            )),
            None => Box::new(backup::Directory(PathBuf::from(target))),
        })
    }

    fn key(&self) -> Result<backup::BackupKey, Box<dyn Error + Send + Sync>> {
        let file = self
            .key_file
            .as_ref()
            .ok_or("name the backup key with --key-file")?;
        Ok(backup::BackupKey::from_hex(&std::fs::read_to_string(
            file,
        )?)?)
    }
}

fn maintain(command: &str, options: Options) -> Result<(), Box<dyn Error + Send + Sync>> {
    match command {
        "backup" => {
            let report = backup::backup(
                &options.data_dir(),
                options.target()?.as_ref(),
                &options.key()?,
                std::time::SystemTime::now(),
            )?;
            tracing::info!(
                snapshot = report.snapshot,
                objects = report.objects,
                uploaded = report.uploaded,
                pruned = report.pruned,
                "backed up"
            );
        }
        "restore" => {
            let data_dir = options
                .data_dir
                .clone()
                .ok_or("name the directory to restore into")?;
            let report = backup::restore(
                options.target()?.as_ref(),
                &options.key()?,
                &data_dir,
                options.snapshot.as_deref(),
            )?;
            tracing::info!(
                snapshot = report.snapshot,
                objects = report.objects,
                "restored"
            );
        }
        _ => {
            let report = backup::verify(&options.data_dir())?;
            if !report.missing.is_empty() {
                return Err(format!(
                    "{} of {} objects are missing or damaged: {}",
                    report.missing.len(),
                    report.objects,
                    report.missing.join(", ")
                )
                .into());
            }
            tracing::info!(
                objects = report.objects,
                "every object is present and intact"
            );
        }
    }
    Ok(())
}
