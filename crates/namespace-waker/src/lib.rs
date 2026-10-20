use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

pub const CLUSTER_KEY: &str = "buildserver";

const FRESH: Duration = Duration::from_secs(60);
const MAX_HEAD_BYTES: usize = 16 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Waker {
    nsc: PathBuf,
    token: PathBuf,
    config: PathBuf,
    answer: Mutex<Option<(Instant, Arc<str>)>>,
}

impl Waker {
    pub fn new(nsc: PathBuf, token: PathBuf, config: PathBuf) -> Self {
        Self {
            nsc,
            token,
            config,
            answer: Mutex::new(None),
        }
    }

    pub async fn answer(&self) -> Result<Arc<str>, String> {
        let mut answer = self.answer.lock().await;
        if let Some((at, text)) = &*answer
            && at.elapsed() < FRESH
        {
            return Ok(Arc::clone(text));
        }
        let (nsc, token, config) = (self.nsc.clone(), self.token.clone(), self.config.clone());
        let text: Arc<str> = tokio::task::spawn_blocking(move || setup(&nsc, &token, &config))
            .await
            .map_err(|error| error.to_string())??
            .into();
        *answer = Some((Instant::now(), Arc::clone(&text)));
        Ok(text)
    }
}

fn setup(nsc: &Path, token: &Path, config: &Path) -> Result<String, String> {
    let _ = fs::remove_file(config);
    let output = Command::new(nsc)
        .env("NS_DO_NOT_UPDATE", "1")
        .args(["reapi", "setup", "buck2", "--token"])
        .arg(token)
        .args(["--key", CLUSTER_KEY, "--config"])
        .arg(config)
        .output()
        .map_err(|error| format!("could not run {}: {error}", nsc.display()))?;
    if !output.status.success() {
        return Err(format!(
            "nsc reapi setup failed ({}):\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let text = fs::read_to_string(config).map_err(|error| {
        format!(
            "nsc wrote no configuration to {}: {error}",
            config.display()
        )
    })?;
    let _ = fs::remove_file(config);
    for key in ["engine_address", "cas_address", "http_headers"] {
        if !text.lines().any(|line| line.trim_start().starts_with(key)) {
            return Err(format!("nsc's configuration does not name {key}:\n{text}"));
        }
    }
    Ok(text)
}

pub async fn serve(
    listener: TcpListener,
    waker: Arc<Waker>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let waker = Arc::clone(&waker);
                tokio::spawn(async move {
                    if let Err(error) = respond(stream, &waker).await {
                        tracing::debug!("namespace request ended: {error}");
                    }
                });
            }
        }
    }
}

async fn respond(mut stream: TcpStream, waker: &Waker) -> io::Result<()> {
    let method = match tokio::time::timeout(READ_TIMEOUT, read_method(&mut stream)).await {
        Ok(method) => method?,
        Err(_) => return Ok(()),
    };
    let (status, body) = if method != "GET" {
        (
            "405 Method Not Allowed",
            "GET asks for the cluster's hosts.\n".to_owned(),
        )
    } else {
        match waker.answer().await {
            Ok(text) => ("200 OK", text.to_string()),
            Err(error) => {
                tracing::error!("{error}");
                ("502 Bad Gateway", format!("{error}\n"))
            }
        }
    };
    let head = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await
}

async fn read_method(stream: &mut TcpStream) -> io::Result<String> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        head.extend_from_slice(&chunk[..read]);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);
        match request.parse(&head) {
            Ok(httparse::Status::Complete(_)) => {
                return Ok(request.method.unwrap_or_default().to_owned());
            }
            Ok(httparse::Status::Partial) if head.len() < MAX_HEAD_BYTES => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bad request head",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests;
