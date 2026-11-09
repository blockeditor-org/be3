//! The NativeLink build server on boat (guides/build_server.md): wake its scheduler VM, register this machine's
//! WireGuard key with it, bring the tunnel up and check that it carries traffic. The scheduler has a fixed address
//! inside the tunnel, so what comes back is a plain host:port for buck2, the same as the `local` server's.
//!
//! One boat key does all of it (`BE3_NL_KEY`, or `nl-key` in ~/.config/be3): it reads and resumes the scheduler
//! sandbox and runs `nl-register-client.sh` on it. Boat limits it to the `nl-scheduler` environment.

use std::io::Write as _;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use be_lib_prompt::{Saved, config_directory, save_private};
use serde_json::{Value, json};

use crate::download::curl_error;
use crate::{Result, repository};

/// The scheduler inside the tunnel, and the port NativeLink serves buck2 on.
pub const ADDRESS: &str = "10.100.0.1:50052";
const TUNNEL: &str = "10.100.0.0/24";
const INTERFACE: &str = "wg-be3";
const API: &str = "https://boat.dev/api/v1";
const SCHEDULER_PREFIX: &str = "nl-sched";
/// Names another scheduler to use, such as a staging one.
const SCHEDULER_VARIABLE: &str = "BE3_NL_SCHEDULER";
const RUNNING: [&str; 3] = ["ready", "idle", "running"];

pub fn key() -> Saved<'static> {
    Saved {
        variable: "BE3_NL_KEY",
        name: "nl-key",
        what: "NativeLink build server boat key",
        checkout: Some(repository()),
    }
}

/// Makes the scheduler reachable from this machine and returns its address for buck2.
pub fn connect() -> Result<String> {
    if !cfg!(target_os = "linux") {
        return Err("The NativeLink build server is reached over WireGuard, which this tool sets up on Linux only.\n\
                    BE3_BUILD_SERVER picks another server (guides/build_server.md)."
            .to_owned());
    }
    // Steady state: the tunnel is up and the scheduler answers.
    if reachable(Duration::from_secs(2)) {
        return Ok(ADDRESS.to_owned());
    }
    let saved = key();
    let why = "buck2 builds on the NativeLink build server, and needs its boat key (guides/build_server.md).";
    let Some(key) = saved.stored().or_else(|| saved.ask(why)) else {
        return Err(
            "buck2 builds on the NativeLink build server, and there is no boat key for it. Set\n\
             BE3_NL_KEY, or write the key to .nl-key at the root of the checkout or to\n\
             ~/.config/be3/nl-key. guides/build_server.md says how to make one."
                .to_owned(),
        );
    };
    let boat = Boat { key };
    eprintln!("Waking the build server's scheduler...");
    let scheduler = boat.wake()?;
    let registration = register(&boat, &scheduler)?;
    bring_up(&registration)?;
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        if reachable(Duration::from_secs(2)) {
            return Ok(ADDRESS.to_owned());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err(format!(
        "The build server's scheduler is awake but this machine cannot reach it over WireGuard ({ADDRESS}).\n\
         Boat places each new VM at random and about half cannot reach the scheduler (a boat network\n\
         problem that does not clear by itself on that VM). Run this on a new VM, or set\n\
         BE3_BUILD_SERVER to another server (guides/build_server.md)."
    ))
}

fn reachable(timeout: Duration) -> bool {
    let Ok(address) = ADDRESS.parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&address, timeout).is_ok()
}

struct Boat {
    key: String,
}

/// What boat reports about the scheduler once it runs.
struct Scheduler {
    id: String,
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
    fn wake(&self) -> Result<Scheduler> {
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
                return Ok(Scheduler { id });
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

/// What the scheduler tells a client it registered.
#[derive(Debug, PartialEq, Eq)]
pub struct Registration {
    pub ip: String,
    pub hub_key: String,
    pub endpoint: String,
}

pub fn parse_registration(output: &str) -> Option<Registration> {
    let value = |name: &str| {
        output
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    Some(Registration {
        ip: value("ip")?,
        hub_key: value("hub_pubkey")?,
        endpoint: value("endpoint")?,
    })
}

/// This machine's name at the scheduler: the same machine keeps its address between builds.
fn client_name() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .filter(|host| !host.is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .unwrap_or_else(|| "client".to_owned());
    let clean: String = host
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    format!("c-{clean}")
}

fn private_key_file() -> std::path::PathBuf {
    config_directory().join("nl-wireguard-key")
}

/// Makes this machine's WireGuard key once, and returns its public half.
fn public_key() -> Result<String> {
    let path = private_key_file();
    if !path.is_file() {
        let output = Command::new("wg")
            .arg("genkey")
            .output()
            .map_err(wireguard_error)?;
        if !output.status.success() {
            return Err(format!(
                "wg genkey failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        save_private(&path, &String::from_utf8_lossy(&output.stdout))
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    }
    let mut child = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(wireguard_error)?;
    let private = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(private.as_bytes());
    }
    let output = child.wait_with_output().map_err(wireguard_error)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn wireguard_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        "wg was not found on PATH. Install WireGuard's tools (sudo apt-get install wireguard-tools).".to_owned()
    } else {
        format!("could not run wg: {error}")
    }
}

fn register(boat: &Boat, scheduler: &Scheduler) -> Result<Registration> {
    let name = client_name();
    let public = public_key()?;
    let command = format!("sudo /usr/local/bin/nl-register-client.sh {name} {public}");
    let (status, answer) = boat.call(
        "POST",
        &format!("/sandboxes/{}/commands", scheduler.id),
        Some(&json!({ "command": command, "timeoutSeconds": 60 })),
    )?;
    if status != 200 {
        return Err(api_error(
            "register this machine with the build server",
            status,
            &answer,
        ));
    }
    if answer["exitCode"].as_i64() != Some(0) {
        return Err(format!(
            "The build server refused to register this machine: {}",
            answer["stderr"].as_str().unwrap_or_default().trim()
        ));
    }
    parse_registration(answer["stdout"].as_str().unwrap_or_default())
        .ok_or_else(|| "The build server's registration answer was not understood.".to_owned())
}

/// Runs a command as root: directly when this is root, else through sudo without a prompt.
fn privileged(program: &str, args: &[&str]) -> Result<()> {
    let root = Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "0");
    let mut command = if root {
        Command::new(program)
    } else {
        let mut command = Command::new("sudo");
        command.args(["-n", program]);
        command
    };
    let output = command
        .args(args)
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{program} {} failed: {}\nThe WireGuard tunnel to the build server needs root, or sudo without a password.",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Creates the tunnel interface if it is missing and points it at the scheduler as it is now.
fn bring_up(registration: &Registration) -> Result<()> {
    let exists = Command::new("ip")
        .args(["link", "show", INTERFACE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !exists {
        privileged("ip", &["link", "add", INTERFACE, "type", "wireguard"])?;
    }
    let key_file = private_key_file();
    let key_file: &Path = key_file.as_path();
    privileged(
        "wg",
        &[
            "set",
            INTERFACE,
            "private-key",
            &key_file.display().to_string(),
            "peer",
            &registration.hub_key,
            "endpoint",
            &registration.endpoint,
            "allowed-ips",
            TUNNEL,
            "persistent-keepalive",
            "25",
        ],
    )?;
    // A different scheduler (a replacement, a staging one) has another key: drop every peer but the current one.
    let stale: Vec<String> = Command::new("wg")
        .args(["show", INTERFACE, "peers"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .filter(|peer| *peer != registration.hub_key)
                .collect()
        })
        .unwrap_or_default();
    for peer in &stale {
        privileged("wg", &["set", INTERFACE, "peer", peer, "remove"])?;
    }
    privileged(
        "ip",
        &[
            "address",
            "replace",
            &format!("{}/24", registration.ip),
            "dev",
            INTERFACE,
        ],
    )?;
    privileged("ip", &["link", "set", INTERFACE, "up"])
}
