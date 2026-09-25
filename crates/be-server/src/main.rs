use std::{env, error::Error, net::SocketAddr, path::PathBuf, process::ExitCode};

use be_server::ServerConfig;
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  be-server [--addr ADDR] [--data-dir DIR] [--disable-registration]
  be-server [--data-dir DIR] --add-account EMAIL NAME PASSWORD WORKSPACE

  --addr ADDR               Address to listen on (default: 127.0.0.1:9090)
  --data-dir DIR            Where accounts and blocks are stored (default: be-server-data)
  --disable-registration    Refuse new accounts; provision them with --add-account instead";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("be-server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let mut address: SocketAddr = "127.0.0.1:9090".parse()?;
    let mut data_dir = PathBuf::from("be-server-data");
    let mut config = ServerConfig::default();
    let mut account: Option<(String, String, String, String)> = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--addr" | "--address" => {
                address = arguments.next().ok_or("--addr needs a value")?.parse()?;
            }
            "--data-dir" => {
                data_dir = PathBuf::from(arguments.next().ok_or("--data-dir needs a value")?);
            }
            "--disable-registration" => config.allow_registration = false,
            "--add-account" => {
                let email = arguments.next().ok_or("--add-account needs an email")?;
                let display_name = arguments.next().ok_or("--add-account needs a name")?;
                let password = arguments.next().ok_or("--add-account needs a password")?;
                let workspace = arguments.next().ok_or("--add-account needs a workspace")?;
                account = Some((email, display_name, password, workspace));
            }
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }

    if let Some((email, display_name, password, workspace)) = account {
        let (account, workspace) =
            be_server::add_account(&data_dir, &email, &display_name, &password, &workspace).await?;
        println!("account {account} owns workspace {workspace}");
        return Ok(());
    }

    let listener = TcpListener::bind(address).await?;
    println!(
        "be-server listening on {} storing data in {}{}",
        listener.local_addr()?,
        data_dir.display(),
        match config.allow_registration {
            true => "",
            false => " (registration disabled)",
        }
    );
    be_server::serve_with_config(listener, data_dir, config, std::future::pending::<()>()).await?;
    Ok(())
}
