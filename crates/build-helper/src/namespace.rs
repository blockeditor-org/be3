use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

pub const CLUSTER_KEY: &str = "buildserver";

pub const LIFETIME: Duration = Duration::from_secs(12600);

const FRESH: Duration = Duration::from_secs(60);

pub(crate) type Probe = Arc<dyn Fn(&Path, &str) -> bool + Send + Sync>;

pub struct Waker {
    nsc: PathBuf,
    token: PathBuf,
    directory: PathBuf,
    pub(crate) probe: Probe,
    pub(crate) answer: Mutex<Option<(Instant, Arc<str>)>>,
}

impl Waker {
    pub fn new(nsc: PathBuf, token: PathBuf, directory: PathBuf) -> Self {
        Self {
            nsc,
            token,
            directory,
            probe: Arc::new(answers),
            answer: Mutex::new(None),
        }
    }

    pub async fn answer(&self) -> Result<(Arc<str>, Duration), String> {
        let mut answer = self.answer.lock().await;
        if let Some((at, text)) = &*answer {
            let age = at.elapsed();
            if age < FRESH {
                return Ok((Arc::clone(text), age));
            }
            if age < LIFETIME {
                let (probe, directory, probed) = (
                    Arc::clone(&self.probe),
                    self.directory.clone(),
                    Arc::clone(text),
                );
                let answering = tokio::task::spawn_blocking(move || probe(&directory, &probed))
                    .await
                    .map_err(|error| error.to_string())?;
                if answering {
                    return Ok((Arc::clone(text), age));
                }
                tracing::info!("the executor stopped answering; asking nsc again");
            }
        }
        let (nsc, token, directory) =
            (self.nsc.clone(), self.token.clone(), self.directory.clone());
        let text: Arc<str> = tokio::task::spawn_blocking(move || setup(&nsc, &token, &directory))
            .await
            .map_err(|error| error.to_string())??
            .into();
        *answer = Some((Instant::now(), Arc::clone(&text)));
        Ok((text, Duration::ZERO))
    }
}

fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim())
    })
}

fn answers(directory: &Path, text: &str) -> bool {
    let (Some(engine), Some(header)) = (value(text, "engine_address"), value(text, "http_headers"))
    else {
        return false;
    };
    let host = engine
        .split_once("://")
        .map_or(engine, |(_, rest)| rest)
        .split([':', '/'])
        .next()
        .unwrap_or_default();
    let body = directory.join("probe-body");
    if fs::write(&body, [0u8; 5]).is_err() {
        return false;
    }
    let config = format!(
        "header = \"{}\"\ndata-binary = \"@{}\"\n",
        header.replacen(':', ": ", 1),
        body.display()
    );
    let Ok(mut curl) = Command::new("curl")
        .args([
            "--silent",
            "--max-time",
            "20",
            "--output",
            "/dev/null",
            "--dump-header",
            "-",
            "--header",
            "content-type: application/grpc",
            "--header",
            "te: trailers",
            "--config",
            "-",
        ])
        .arg(format!(
            "https://{host}/build.bazel.remote.execution.v2.Capabilities/GetCapabilities"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = curl.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let Ok(output) = curl.wait_with_output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("content-type")
                && value
                    .trim()
                    .to_ascii_lowercase()
                    .starts_with("application/grpc")
        })
    })
}

fn setup(nsc: &Path, token: &Path, directory: &Path) -> Result<String, String> {
    fs::create_dir_all(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let config = &directory.join("buck2.buckconfig");
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
        if value(&text, key).is_none() {
            return Err(format!("nsc's configuration does not name {key}:\n{text}"));
        }
    }
    Ok(text)
}
