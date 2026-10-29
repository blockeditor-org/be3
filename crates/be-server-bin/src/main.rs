use std::{env, error::Error, net::SocketAddr, path::PathBuf, process::ExitCode};

use be_server::{ServerConfig, backup};
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  be-server [--addr ADDR] [--data-dir DIR] [--allow-login] [--allow-registration]
            [--build-info-file FILE] [--auth-origin URL]
  be-server backup-key > KEY-FILE
  be-server backup [--data-dir DIR] --to TARGET --key-file KEY-FILE
  be-server restore --from TARGET --key-file KEY-FILE --data-dir EMPTY-DIR [--snapshot NAME]
  be-server verify [--data-dir DIR]

  --addr ADDR               Address to listen on (default: 127.0.0.1:9090)
  --data-dir DIR            Where accounts and blocks are stored (default: be-server-data)
  --allow-login             Accept sign-ins with a password; without it, only devices
                            already signed in can connect, with the tokens they hold
  --allow-registration      Let anyone who reaches the portal register a verified account
  --build-info-file FILE    JSON build metadata; clients must match its id. By
                            default the build-info.json file beside the executable, and with
                            neither it does not start
  --auth-origin URL         Public origin of the account portal (required outside localhost)

A backup uploads every object the database references that the target does not
have yet, never deleting one, and then the database, sealed with the backup
key. It keeps every database from the last 48 hours, one a day for 30 days and
one a month for a year. TARGET is a directory, or bunny:ZONE for a Bunny
Storage zone, with the zone's password in BUNNY_STORAGE_KEY and its region's
endpoint in BUNNY_STORAGE_ENDPOINT (default: https://storage.bunnycdn.com).
Without the backup key a backup cannot be restored, and without the recovery
phrase its notes cannot be read.

RUST_LOG sets what is logged (default: info).";

macro_rules! assets {
    ($($name:literal: $content_type:literal),* $(,)?) => {
        &[$(be_portal::Asset {
            name: $name,
            content_type: $content_type,
            bytes: include_bytes!(concat!(env!("BE_PORTAL_ASSETS"), "/", $name)),
        }),*]
    };
}

const PORTAL: be_portal::Portal = be_portal::Portal {
    assets: assets! {
        "start.js": "text/javascript",
        "be_portal_ui.js": "text/javascript",
        "be_portal_ui_bg.wasm": "application/wasm",
        "wasi.js": "text/javascript",
        "threads.js": "text/javascript",
        "thread.js": "text/javascript",
        "dylink.js": "text/javascript",
        "env.js": "text/javascript",
        "Inter-Regular.ttf": "font/ttf",
        "Hack-Regular.ttf": "font/ttf",
        "NotoEmoji-Regular.ttf": "font/ttf",
        "lucide.ttf": "font/ttf",
    },
    ..be_portal_ui::PORTAL
};

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
    let mut config = ServerConfig {
        portal: Some(PORTAL),
        ..ServerConfig::default()
    };

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--addr" | "--address" => {
                address = arguments.next().ok_or("--addr needs a value")?.parse()?;
            }
            "--data-dir" => {
                data_dir = PathBuf::from(arguments.next().ok_or("--data-dir needs a value")?);
            }
            "--allow-registration" => config.allow_registration = true,
            "--allow-login" => config.allow_login = true,
            "--build-info-file" => {
                build_id::install_from(&PathBuf::from(
                    arguments.next().ok_or("--build-info-file needs a value")?,
                ))?;
            }
            "--auth-origin" => {
                let origin = arguments.next().ok_or("--auth-origin needs a URL")?;
                be_server::mail::validate_origin(&origin)?;
                config.auth_origin = Some(origin);
            }
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }

    let build = build_id::current()?;
    let listener = TcpListener::bind(address).await?;
    tracing::info!(
        address = %listener.local_addr()?,
        data = %data_dir.display(),
        login = config.allow_login,
        registration = config.allow_registration,
        build,
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
