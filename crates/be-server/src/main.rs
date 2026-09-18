use std::{env, error::Error, net::SocketAddr, path::PathBuf, process::ExitCode};

use tokio::net::TcpListener;

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
    let mut address: SocketAddr = "127.0.0.1:8787".parse()?;
    let mut data_dir = PathBuf::from("be-server-data");
    let mut account: Option<(String, String, String, String)> = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--address" => address = arguments.next().ok_or("--address needs a value")?.parse()?,
            "--data-dir" => {
                data_dir = PathBuf::from(arguments.next().ok_or("--data-dir needs a value")?);
            }
            "--add-account" => {
                let email = arguments.next().ok_or("--add-account needs an email")?;
                let display_name = arguments.next().ok_or("--add-account needs a name")?;
                let password = arguments.next().ok_or("--add-account needs a password")?;
                let workspace = arguments.next().ok_or("--add-account needs a workspace")?;
                account = Some((email, display_name, password, workspace));
            }
            other => return Err(format!("unknown argument {other}").into()),
        }
    }

    if let Some((email, display_name, password, workspace)) = account {
        let (account, workspace) =
            be_server::add_account(&data_dir, &email, &display_name, &password, &workspace).await?;
        println!("account {account} owns workspace {workspace}");
        return Ok(());
    }

    let listener = TcpListener::bind(address).await?;
    println!("be-server listening on {}", listener.local_addr()?);
    be_server::serve(listener, data_dir).await?;
    Ok(())
}
