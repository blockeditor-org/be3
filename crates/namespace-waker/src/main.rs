use std::{env, error::Error, net::SocketAddr, path::PathBuf, process::ExitCode, sync::Arc};

use namespace_waker::{Waker, serve};
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  namespace-waker --token FILE [--addr ADDR] [--nsc PATH]

  --token FILE   Namespace's token, as `nsc reapi create-token` writes it
  --addr ADDR    Address to listen on (default: 127.0.0.1:9091)
  --nsc PATH     The nsc to run (default: nsc); scripts/internal/install-nsc.sh
                 downloads the pinned one

Answers every GET with the buck2 configuration of Namespace's remote execution
cluster, waking it first: what `nsc reapi setup buck2` writes. Callers within a
minute of a setup share its answer. It checks no credentials: the reverse proxy
in front of it does (guides/build_server.md).

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
    let mut address: SocketAddr = "127.0.0.1:9091".parse()?;
    let mut nsc = PathBuf::from("nsc");
    let mut token = None;
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--addr" | "--address" => address = value()?.parse()?,
            "--nsc" => nsc = PathBuf::from(value()?),
            "--token" => token = Some(PathBuf::from(value()?)),
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }
    let token = token.ok_or(format!(
        "name Namespace's token file with --token\n\n{USAGE}"
    ))?;
    let config = env::temp_dir().join(format!("namespace-waker-{}.buckconfig", std::process::id()));
    let waker = Arc::new(Waker::new(nsc, token, config));
    let listener = TcpListener::bind(address).await?;
    tracing::info!(address = %listener.local_addr()?, "namespace-waker listening");
    serve(listener, waker, shutdown()).await?;
    Ok(())
}

async fn shutdown() {
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
