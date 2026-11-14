use std::{env, error::Error, net::SocketAddr, path::PathBuf, process::ExitCode};

use be_server::{ServerConfig, backup};
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  be-server [--addr ADDR] [--data-dir DIR] [--allow-login] [--allow-registration]
            [--build-info-file FILE] [--auth-origin URL] [--account-quota BYTES]
            [--trusted-proxy ADDR]... [--object-cache BYTES] [--public-url URL]
            [--fcm-credentials FILE] [--push-host HOST]... [--allow-local-push-endpoints]
  be-server backup-key > KEY-FILE
  be-server backup [--data-dir DIR] --to TARGET --key-file KEY-FILE
  be-server restore --from TARGET --key-file KEY-FILE --data-dir EMPTY-DIR [--snapshot NAME]
  be-server verify [--data-dir DIR]
  be-server invite-code create [--data-dir DIR] [--uses N] [--expires DAYS]
  be-server invite-code list [--data-dir DIR]
  be-server invite-code revoke [--data-dir DIR] ID

  --addr ADDR               Address to listen on (default: 127.0.0.1:9090)
  --data-dir DIR            Where accounts and blocks are stored (default: be-server-data)
  --allow-login             Accept sign-ins with a password; without it, only devices
                            already signed in can connect, with the tokens they hold
  --allow-registration      Let anyone with an invite code register a verified account
  --build-info-file FILE    JSON build metadata; clients must match its id. By
                            default the build-info.json file beside the executable, and with
                            neither it does not start
  --auth-origin URL         Public origin of the account portal (required outside localhost)
  --account-quota BYTES     Bytes each account may store, or none for no limit
                            (default: 10737418240, 10 GiB)
  --trusted-proxy ADDR      Also take the client's address from X-Forwarded-For when a
                            connection comes from ADDR; it is always taken from loopback, where
                            the reverse proxy runs, and ignored from anyone else
  --object-cache BYTES      With a bucket, the bytes of objects kept in DIR/object-cache
                            (default: 2147483648, 2 GiB)
  --public-url URL          The URL clients reach the server at, with its path behind a
                            proxy (https://api.be3.pfg.pw/api/be). Without a bucket, clients
                            move object bytes through the server's S3 endpoint below it
                            (default: http:// and the address it listens on)
  --fcm-credentials FILE    A Firebase project's service account (its JSON key file), to
                            push notifications to Android devices through FCM; without it,
                            notifications go only through Web Push
  --push-host HOST          Also push to Web Push endpoints on HOST, exactly: a self-hosted
                            UnifiedPush server such as ntfy. Without, only the known push
                            services (FCM, Mozilla, Apple, Windows) are pushed to
  --allow-local-push-endpoints
                            Let Web Push endpoints be http:// and on loopback or private
                            addresses, for a test that serves one with --push-host; without,
                            an endpoint is https:// on a public address

Objects are kept in DIR/objects, or, when BE_S3_ENDPOINT, BE_S3_REGION,
BE_S3_BUCKET, BE_S3_ACCESS_KEY and BE_S3_SECRET_KEY are all set, in that bucket
of an S3-compatible store (path-style), with the objects used most recently
cached on disk. backup, restore and verify read the same variables.

A backup uploads every object the database references that the target does not
have yet, never deleting one, and then the database, sealed with the backup
key. It keeps every database from the last 48 hours, one a day for 30 days and
one a month for a year. TARGET is a directory, or bunny:ZONE for a Bunny
Storage zone, with the zone's password in BUNNY_STORAGE_KEY and its region's
endpoint in BUNNY_STORAGE_ENDPOINT (default: https://storage.bunnycdn.com).
Without the backup key a backup cannot be restored, and without the recovery
phrase its notes cannot be read.

Registering needs an invite code from `invite-code create`, which prints the
code once; the server keeps only its hash. --uses limits how many accounts it
makes and --expires how many days it works (by default neither is limited).
`list` shows each code's id, uses and expiry, and `revoke ID` deletes one.
These work on the data directory while the server runs.

RUST_LOG sets what is logged (default: info).";

macro_rules! assets {
    ($($name:literal: $content_type:literal),* $(,)?) => {
        &[$(be_server_portal::Asset {
            name: $name,
            content_type: $content_type,
            bytes: include_bytes!(concat!(env!("BE_PORTAL_ASSETS"), "/", $name)),
        }),*]
    };
}

const PORTAL: be_server_portal::Portal = be_server_portal::Portal {
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
    ..be_server_portal_ui::PORTAL
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

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut arguments = env::args().skip(1).peekable();
    match arguments.peek().map(String::as_str) {
        Some("backup-key") => {
            println!("{}", backup::BackupKey::generate().to_hex());
            return Ok(());
        }
        Some("invite-code") => {
            arguments.next();
            return invite_code(arguments).map_err(|error| error.to_string().into());
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
    let mut cache_bytes = be_server::objects::DEFAULT_CACHE_BYTES;

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
                be_lib_build_id::install_from(&PathBuf::from(
                    arguments.next().ok_or("--build-info-file needs a value")?,
                ))?;
            }
            "--auth-origin" => {
                let origin = arguments.next().ok_or("--auth-origin needs a URL")?;
                be_server::mail::validate_origin(&origin)?;
                config.auth_origin = Some(origin);
            }
            "--account-quota" => {
                let quota = arguments
                    .next()
                    .ok_or("--account-quota needs a byte count")?;
                config.account_quota = match quota.as_str() {
                    "none" => None,
                    bytes => Some(bytes.parse()?),
                };
            }
            "--trusted-proxy" => {
                let proxy = arguments
                    .next()
                    .ok_or("--trusted-proxy needs an address")?
                    .parse()?;
                config.trusted_proxies.to_mut().push(proxy);
            }
            "--public-url" => {
                config.public_url = Some(arguments.next().ok_or("--public-url needs a URL")?);
            }
            "--fcm-credentials" => {
                let file = arguments.next().ok_or("--fcm-credentials needs a file")?;
                let json = std::fs::read_to_string(&file)
                    .map_err(|error| format!("reading {file}: {error}"))?;
                config.push.fcm = Some(be_server::push::FcmConfig::from_service_account(&json)?);
            }
            "--push-host" => {
                let host = arguments.next().ok_or("--push-host needs a host name")?;
                let host = host.trim_end_matches('.').to_ascii_lowercase();
                if host.is_empty() || host.contains([':', '/', '@', '\\', ' ']) {
                    return Err(format!(
                        "--push-host takes a bare host name, not {host:?} (no scheme, port or path)"
                    )
                    .into());
                }
                config.push.hosts.to_mut().push(host);
            }
            "--allow-local-push-endpoints" => config.push.allow_local_endpoints = true,
            "--object-cache" => {
                cache_bytes = arguments
                    .next()
                    .ok_or("--object-cache needs a byte count")?
                    .parse()?;
            }
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }
    config.bucket = bucket(cache_bytes).map_err(|error| error.to_string())?;

    let build = be_lib_build_id::current()?;
    let listener = TcpListener::bind(address).await?;
    tracing::info!(
        address = %listener.local_addr()?,
        data = %data_dir.display(),
        login = config.allow_login,
        registration = config.allow_registration,
        bucket = ?config.bucket.as_ref().map(|config| &config.bucket),
        public_url = ?config.public_url,
        fcm = config.push.fcm.is_some(),
        build,
        "be-server listening"
    );
    be_server::serve_with_config(listener, data_dir, config, shutdown()).await?;
    tracing::info!("be-server stopped");
    Ok(())
}

/// The bucket the `BE_S3_*` variables name, if they are set: all five, or none.
fn bucket(
    cache_bytes: u64,
) -> Result<Option<be_server::objects::BucketConfig>, Box<dyn Error + Send + Sync>> {
    const NAMES: [&str; 5] = [
        "BE_S3_ENDPOINT",
        "BE_S3_REGION",
        "BE_S3_BUCKET",
        "BE_S3_ACCESS_KEY",
        "BE_S3_SECRET_KEY",
    ];
    let values: Vec<Option<String>> = NAMES.iter().map(|name| env::var(name).ok()).collect();
    if values.iter().all(Option::is_none) {
        return Ok(None);
    }
    let [
        Some(endpoint),
        Some(region),
        Some(name),
        Some(access),
        Some(secret),
    ] = <[Option<String>; 5]>::try_from(values).expect("five names")
    else {
        return Err(format!("set all of {} or none", NAMES.join(", ")).into());
    };
    Ok(Some(be_server::objects::BucketConfig {
        bucket: be_server::objects::Bucket::new(&endpoint, &region, &name, &access, &secret)?,
        cache_bytes,
    }))
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
                bucket(be_server::objects::DEFAULT_CACHE_BYTES)?.as_ref(),
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
                bucket(be_server::objects::DEFAULT_CACHE_BYTES)?.as_ref(),
                options.snapshot.as_deref(),
            )?;
            tracing::info!(
                snapshot = report.snapshot,
                objects = report.objects,
                "restored"
            );
        }
        _ => {
            let report = backup::verify(
                &options.data_dir(),
                bucket(be_server::objects::DEFAULT_CACHE_BYTES)?.as_ref(),
            )?;
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

fn invite_code(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use be_server::store::invite_codes;
    let command = arguments
        .next()
        .ok_or(format!("name an invite-code command\n\n{USAGE}"))?;
    let mut data_dir = PathBuf::from("be-server-data");
    let mut uses = None;
    let mut expires = None;
    let mut id = None;
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--data-dir" => data_dir = PathBuf::from(value()?),
            "--uses" if command == "create" => uses = Some(value()?.parse::<u32>()?),
            "--expires" if command == "create" => expires = Some(value()?.parse::<u32>()?),
            other if command == "revoke" && id.is_none() && !other.starts_with('-') => {
                id = Some(other.to_owned());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }
    let shown =
        |limit: Option<i64>, none: &str| limit.map_or(none.to_owned(), |limit| limit.to_string());
    match command.as_str() {
        "create" => {
            let (code, record) = invite_codes::create(&data_dir, uses, expires)?;
            println!("{code}");
            eprintln!(
                "invite code {} made: uses {}, expires {}",
                record.id,
                shown(record.uses, "unlimited"),
                shown(record.expires, "never")
            );
        }
        "list" => {
            println!("id        created     used  uses       expires");
            for code in invite_codes::list(&data_dir)? {
                println!(
                    "{:<8}  {:<10}  {:<4}  {:<9}  {}",
                    code.id,
                    code.created,
                    code.used,
                    shown(code.uses, "unlimited"),
                    shown(code.expires, "never")
                );
            }
        }
        "revoke" => {
            let id = id.ok_or("name the code to revoke by its id")?;
            if !invite_codes::revoke(&data_dir, &id)? {
                return Err(format!("no invite code has the id {id}").into());
            }
            eprintln!("invite code {id} revoked");
        }
        other => return Err(format!("unknown invite-code command {other}\n\n{USAGE}").into()),
    }
    Ok(())
}
