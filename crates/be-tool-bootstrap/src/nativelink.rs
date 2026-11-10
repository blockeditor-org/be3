//! The NativeLink build server on boat (guides/build_server.md). Boat hosts the scheduler's port 8099 publicly, where
//! Caddy checks a bearer token and forwards each call to NativeLink. Boat's edge carries gRPC only as HTTP/1.1, so buck2
//! always reaches it through scripts/internal/re-relay with `-http1`, which also takes an HTTPS proxy's way out. That
//! needs no root, no WireGuard and no IPv6, so it works on a GitHub-hosted runner and in a Claude Code cloud session.
//!
//! Two secrets: `BE3_NL_URL` is the hosted port's address with Caddy's token as `?_token=`, which buck2 sends as
//! `Authorization: Bearer`. Without a token no header is sent, for a proxy that adds it (a cloud session's).
//! `BE3_NL_KEY` is a boat key (limited to the `nl-scheduler` environment, actions `sandbox.read` and
//! `sandbox.resume`) that wakes the scheduler when boat has stopped it. A running scheduler needs only the address.

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use be_lib_prompt::Saved;
use serde_json::{Value, json};

use crate::download::curl_error;
use crate::{Result, repository};

const API: &str = "https://boat.dev/api/v1";
const SCHEDULER_PREFIX: &str = "nl-sched";
/// Names another scheduler to wake, such as a staging one.
const SCHEDULER_VARIABLE: &str = "BE3_NL_SCHEDULER";
const RUNNING: [&str; 3] = ["ready", "idle", "running"];

pub fn address() -> Saved<'static> {
    Saved {
        variable: "BE3_NL_URL",
        name: "nl-url",
        what: "NativeLink build server address",
        checkout: Some(repository()),
    }
}

pub fn key() -> Saved<'static> {
    Saved {
        variable: "BE3_NL_KEY",
        name: "nl-key",
        what: "NativeLink build server boat key",
        checkout: Some(repository()),
    }
}

/// The scheduler's host and its token (empty when a proxy adds it), once it answers; wakes it if boat stopped it.
pub fn connect() -> Result<(String, String)> {
    let saved = address();
    let why = "buck2 builds on the NativeLink build server, and needs its address (guides/build_server.md).";
    let Some(url) = saved.stored().or_else(|| saved.ask(why)) else {
        return Err("buck2 builds on the NativeLink build server, and there is no address for it. Set\n\
             BE3_NL_URL, or write it to .nl-url at the root of the checkout or to ~/.config/be3/nl-url.\n\
             guides/build_server.md says where to get it."
            .to_owned());
    };
    let (host, token) = parse_address(&url)?;
    let first = match probe(&host, &token)? {
        None => return Ok((host, token)),
        Some(error) => error,
    };
    // Not answering: boat may have stopped the scheduler. Wake it if there is a key to.
    let key = key();
    let Some(key) = key.stored() else {
        return Err(format!(
            "The build server did not answer: {first}\n\
             If boat stopped the scheduler, BE3_NL_KEY (a boat key that can resume it) wakes it\n\
             (guides/build_server.md)."
        ));
    };
    eprintln!("Waking the build server's scheduler...");
    Boat { key }.wake()?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut last = first;
    while Instant::now() < deadline {
        match probe(&host, &token)? {
            None => return Ok((host, token)),
            Some(error) => last = error,
        }
        std::thread::sleep(Duration::from_secs(4));
    }
    Err(format!(
        "The build server's scheduler is awake but did not answer: {last}\n\
         Boat can take a minute to route to a resumed VM; run it again, or set BE3_BUILD_SERVER to another \
         server (guides/build_server.md)."
    ))
}

/// One request to the scheduler's front: `None` when it answers, the reason when it does not. A refused token is an
/// error at once, since waking cannot help. The token goes to curl on stdin, never on its command line.
fn probe(host: &str, token: &str) -> Result<Option<String>> {
    let mut config = format!(
        "url = \"https://{host}/\"\nsilent\nshow-error\nmax-time = 30\noutput = \"{}\"\n\
         write-out = \"%{{http_code}}\"\n",
        if cfg!(windows) { "NUL" } else { "/dev/null" }
    );
    if !token.is_empty() {
        config.push_str(&format!(
            "header = \"Authorization: Bearer {}\"\n",
            curl_quote(token)
        ));
    }
    let mut child = Command::new("curl")
        .args(["--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(curl_error)?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let output = child.wait_with_output().map_err(curl_error)?;
    let status: u16 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    Ok(match status {
        401 => {
            return Err(format!(
                "The build server at {host} refused its token. BE3_NL_URL's ?_token= must be the scheduler's\n\
                 NL_TUNNEL_TOKEN, or, in a cloud session, the proxy's network secret for {host} (guides/build_server.md)."
            ));
        }
        0 => Some(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
        status if answers(status) => None,
        _ => Some(format!("{host} answered {status}")),
    })
}

/// Whether an HTTP status from the scheduler's front means NativeLink is there. Caddy and NativeLink answer a plain
/// request with a 4xx; a redirect is boat's own answer for a hosted port whose sandbox is stopped (it sends the
/// browser to a fallback page), so it means the scheduler needs waking.
pub fn answers(status: u16) -> bool {
    matches!(status, 200..=299 | 400..=499)
}

/// The scheduler's host and Caddy's token, from `BE3_NL_URL` (`https://NAME-8099.on.boat.dev/?_token=TOKEN`).
pub fn parse_address(url: &str) -> Result<(String, String)> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "BE3_NL_URL is not an address: it should start with https://".to_owned())?;
    let (location, query) = rest.split_once('?').unwrap_or((rest, ""));
    let token = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("_token="))
        .unwrap_or_default();
    let host = location.trim_end_matches('/');
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return Err("BE3_NL_URL should be https://HOST/ with no path or port".to_owned());
    }
    Ok((host.to_owned(), token.to_owned()))
}

struct Boat {
    key: String,
}

impl Boat {
    /// One call to boat's HTTP API through curl. The key goes to curl on stdin, never on its command line.
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value)> {
        let mut config = format!(
            "url = \"{API}{path}\"\nrequest = \"{method}\"\nheader = \"Authorization: Bearer {}\"\n\
             header = \"Content-Type: application/json\"\nsilent\nshow-error\nmax-time = 150\n\
             write-out = \"\\n%{{http_code}}\"\n",
            self.key
        );
        if let Some(body) = body {
            config.push_str(&format!("data = \"{}\"\n", curl_quote(&body.to_string())));
        }
        let mut child = Command::new("curl")
            .args(["--config", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(curl_error)?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(config.as_bytes());
        }
        let output = child.wait_with_output().map_err(curl_error)?;
        let text = String::from_utf8_lossy(&output.stdout);
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", "0"));
        let status: u16 = code.trim().parse().unwrap_or(0);
        if status == 0 {
            return Err(format!(
                "Could not reach boat's API: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok((status, serde_json::from_str(body).unwrap_or(Value::Null)))
    }

    /// Finds the scheduler sandbox and resumes it if it is stopped.
    fn wake(&self) -> Result<()> {
        let prefix = std::env::var(SCHEDULER_VARIABLE)
            .ok()
            .filter(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
            .unwrap_or_else(|| SCHEDULER_PREFIX.to_owned());
        let (status, listing) = self.call(
            "GET",
            &format!("/sandboxes?namePrefix={prefix}&limit=50"),
            None,
        )?;
        if status != 200 {
            return Err(api_error(
                "list the build server's sandboxes",
                status,
                &listing,
            ));
        }
        let sandboxes = listing["sandboxes"].as_array().cloned().unwrap_or_default();
        let Some(chosen) = pick_scheduler(&sandboxes) else {
            return Err(format!(
                "The boat key sees no sandbox named {prefix}*; the key must be limited to the nl-scheduler environment."
            ));
        };
        let id = chosen["id"].as_str().unwrap_or_default().to_owned();
        let running =
            |sandbox: &Value| RUNNING.contains(&sandbox["state"].as_str().unwrap_or_default());
        if !running(chosen) {
            let (status, answer) =
                self.call("POST", &format!("/sandboxes/{id}/resume"), Some(&json!({})))?;
            if status >= 300 {
                return Err(api_error(
                    "resume the build server's scheduler",
                    status,
                    &answer,
                ));
            }
        }
        let deadline = Instant::now() + Duration::from_secs(240);
        loop {
            let (status, info) = self.call("GET", &format!("/sandboxes/{id}"), None)?;
            let sandbox = info.get("sandbox").unwrap_or(&info);
            if status == 200 && running(sandbox) {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(
                    "The build server's scheduler did not start within four minutes.".to_owned(),
                );
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

fn api_error(what: &str, status: u16, answer: &Value) -> String {
    format!(
        "Could not {what} (HTTP {status}): {}",
        answer["error"]
            .as_str()
            .or(answer["message"].as_str())
            .unwrap_or("no reason given")
    )
}

/// The running scheduler if there is one, else the first that exists.
pub fn pick_scheduler(sandboxes: &[Value]) -> Option<&Value> {
    let live = |sandbox: &&Value| RUNNING.contains(&sandbox["state"].as_str().unwrap_or_default());
    sandboxes.iter().find(live).or_else(|| {
        sandboxes
            .iter()
            .find(|sandbox| sandbox["state"].as_str() != Some("deleting"))
    })
}

/// Quotes text for a double-quoted value in a curl config file.
fn curl_quote(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
