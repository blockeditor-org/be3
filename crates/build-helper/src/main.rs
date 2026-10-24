use std::{env, error::Error, net::SocketAddr, path::PathBuf, process::ExitCode, sync::Arc};

use build_helper::{Service, Waker, host::Host, queue::Builds, serve};
use tokio::net::TcpListener;

const USAGE: &str = "\
Usage:
  build-helper --checkout DIR --state DIR [--addr ADDR] [--keystore FILE]
               [--public URL] [--max-buck-out-gb N] [--token FILE [--nsc PATH]]

  --checkout DIR        A clone of the repository the helper owns: it fetches,
                        checks out and cleans it for every build
  --state DIR           Where build logs, manifests and what was uploaded are kept
  --addr ADDR           Address to listen on (default: 127.0.0.1:9091)
  --keystore FILE       The Android keystore APKs are signed with (JKS, PKCS12 or
                        base64 of one, password android); without it, Android
                        builds fail
  --public URL          The CDN that serves the storage zone
                        (default: https://be3-ci.b-cdn.net)
  --max-buck-out-gb N   Run `./scripts/buck clean` after a build once buck-out
                        is larger than this (default: 40)
  --token FILE          Namespace's token, as `nsc reapi create-token` writes it;
                        serves GET /build/namespace, Namespace's buck2 configuration
  --nsc PATH            The nsc to run (default: nsc)

Builds products of commits on the repository's branches on request with
./scripts/buck, one at a time, uploads their files to the be3-ci Bunny storage
zone by SHA-256 and publishes a manifest of them (guides/build_server.md). It
checks no credentials: the reverse proxy in front of it does.

BUNNY_STORAGE_KEY is the storage zone's key; without it nothing is uploaded and
each manifest is only written under the state directory.
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
    let (mut token, mut checkout, mut state, mut keystore) = (None, None, None, None);
    let mut public = "https://be3-ci.b-cdn.net".to_owned();
    let mut max_buck_out_gb: u64 = 40;
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--addr" | "--address" => address = value()?.parse()?,
            "--nsc" => nsc = PathBuf::from(value()?),
            "--token" => token = Some(PathBuf::from(value()?)),
            "--checkout" => checkout = Some(PathBuf::from(value()?)),
            "--state" => state = Some(PathBuf::from(value()?)),
            "--keystore" => keystore = Some(PathBuf::from(value()?)),
            "--public" => public = value()?.trim_end_matches('/').to_owned(),
            "--max-buck-out-gb" => max_buck_out_gb = value()?.parse()?,
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}").into()),
        }
    }
    let checkout = std::path::absolute(checkout.ok_or(format!(
        "name the helper's checkout with --checkout\n\n{USAGE}"
    ))?)?;
    let state = std::path::absolute(
        state.ok_or(format!("name a state directory with --state\n\n{USAGE}"))?,
    )?;
    if !checkout.join(".git").exists() {
        return Err(format!("{} is not a git checkout", checkout.display()).into());
    }
    std::fs::create_dir_all(&state)?;
    let keystore = keystore.map(std::path::absolute).transpose()?;
    let storage_key = env::var("BUNNY_STORAGE_KEY")
        .ok()
        .filter(|key| !key.is_empty());
    if storage_key.is_none() {
        tracing::warn!("BUNNY_STORAGE_KEY is not set; builds will upload nothing");
    }
    let host = Host {
        checkout,
        state: state.clone(),
        keystore,
        public: public.clone(),
        storage_key,
        max_buck_out: max_buck_out_gb << 30,
        git: Default::default(),
    };
    let waker = token.map(|token| {
        let directory = env::temp_dir().join(format!("build-helper-{}", std::process::id()));
        Arc::new(Waker::new(nsc, token, directory))
    });
    let service = Arc::new(Service {
        waker,
        builds: Some(Builds::new(Arc::new(host), &state, public)),
    });
    let listener = TcpListener::bind(address).await?;
    tracing::info!(address = %listener.local_addr()?, "build-helper listening");
    serve(listener, service, shutdown()).await?;
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
