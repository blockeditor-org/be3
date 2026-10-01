use std::{env, error::Error, io::BufRead, net::SocketAddr, path::PathBuf, process::ExitCode};

use be_server::ServerConfig;
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  be-server [--addr ADDR] [--data-dir DIR] [--allow-registration]
  be-server [--data-dir DIR] --add-account EMAIL NAME WORKSPACE < password

  --addr ADDR               Address to listen on (default: 127.0.0.1:9090)
  --data-dir DIR            Where accounts and blocks are stored (default: be-server-data)
  --allow-registration      Let anyone who reaches the server make an account; without it,
                            accounts are made with --add-account
  --add-account             Make an account that owns a new workspace, reading its password
                            from the first line of standard input

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
    let mut arguments = env::args().skip(1);
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
