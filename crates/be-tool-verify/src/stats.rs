//! A record of each ./scripts/verify and ./scripts/ci build, sent to the build server's dashboard
//! (scripts/internal/nativelink/dashboard, guides/build_server.md): how long buck2 took, what ran and what was
//! cached, the critical path, every test run and every other action over 10 s. buck2's event log has it all.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// What the event log does not say about a build.
pub struct Context {
    pub kind: String,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub parts: String,
    pub passed: bool,
}

/// An action other than a test run is in the record when it ran for at least this long.
const SLOW_US: u64 = 10_000_000;
/// A step of the critical path is in the record when it took at least this long.
const CRITICAL_US: u64 = 1_000_000;

/// The record of the build the event log, written as JSON lines, describes, or None when it has no events.
pub fn record(events: &str, context: &Context) -> Option<Value> {
    let (mut id, mut first, mut last) = (None, None, None);
    let (mut cached, mut remote, mut local) = (0u64, 0u64, 0u64);
    let (mut remote_us, mut queue_us) = (0u64, 0u64);
    let mut slow = Vec::new();
    let mut critical = Vec::new();
    for line in events.lines() {
        let action = line.contains("\"SpanEnd\"") && line.contains("\"ActionExecution\"");
        let graph = line.contains("\"BuildGraphInfo\"");
        let timestamped = line.starts_with("{\"Event\"");
        if !timestamped {
            continue;
        }
        // Only the first event, the last and the ones counted below are worth parsing.
        if first.is_some() && !action && !graph {
            last = Some(line);
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event = &event["Event"];
        if first.is_none() {
            first = seconds(&event["timestamp"]);
            id = event["trace_id"].as_str().map(str::to_owned);
        }
        last = Some(line);
        if graph {
            for step in event["data"]["Instant"]["data"]["BuildGraphInfo"]["critical_path2"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let duration = step["duration_us"].as_u64().unwrap_or(0);
                if duration >= CRITICAL_US {
                    critical.push(json!({
                        "what": describe(&step["entry"]),
                        "seconds": duration as f64 / 1e6,
                        "queue_seconds": step["queue_duration_us"].as_u64().map(|us| us as f64 / 1e6),
                    }));
                }
            }
            continue;
        }
        let action = &event["data"]["SpanEnd"]["data"]["ActionExecution"];
        // buck2's ActionExecutionKind: 1, 7 and 8 ran here, 2 on the server, and 3, 9 and 10 were cached; the rest
        // (writing a file, a symlink) are not actions anything ran.
        let ran = match action["execution_kind"].as_u64() {
            Some(1 | 7 | 8) => {
                local += 1;
                true
            }
            Some(2) => {
                remote += 1;
                true
            }
            Some(3 | 9 | 10) => {
                cached += 1;
                false
            }
            _ => false,
        };
        if !ran {
            continue;
        }
        let wall = action["wall_time_us"].as_u64().unwrap_or(0);
        let queue = action["commands"]
            .as_array()
            .and_then(|commands| commands.last())
            .and_then(|command| command["details"]["metadata"]["queue_duration_us"].as_u64());
        if action["execution_kind"] == 2 {
            remote_us += wall;
            queue_us += queue.unwrap_or(0);
        }
        let category = action["name"]["category"].as_str().unwrap_or("?");
        if category == "test_run" || wall >= SLOW_US {
            slow.push(json!({
                "target": target(&action["key"]["owner"]),
                "category": category,
                "seconds": wall as f64 / 1e6,
                "queue_seconds": queue.map(|us| us as f64 / 1e6),
                "failed": action["failed"] == true,
            }));
        }
    }
    let started = first?;
    let finished = last
        .and_then(|line| serde_json::from_str::<Value>(line).ok())
        .and_then(|event| seconds(&event["Event"]["timestamp"]))
        .unwrap_or(started);
    Some(json!({
        "id": id?,
        "kind": context.kind,
        "commit": context.commit,
        "branch": context.branch,
        "parts": context.parts,
        "passed": context.passed,
        "started": started,
        "seconds": finished - started,
        "actions": {"total": cached + remote + local, "cached": cached, "remote": remote, "local": local},
        "remote_seconds": remote_us as f64 / 1e6,
        "queue_seconds": queue_us as f64 / 1e6,
        "critical_path": critical,
        "slow": slow,
    }))
}

/// buck2's `[seconds, nanoseconds]`.
fn seconds(timestamp: &Value) -> Option<f64> {
    Some(timestamp[0].as_f64()? + timestamp[1].as_f64().unwrap_or(0.0) / 1e9)
}

/// `crates/a:test` for an action's owner.
fn target(owner: &Value) -> String {
    let label = &owner["TargetLabel"]["label"];
    let package = label["package"].as_str().unwrap_or("?");
    let package = package.strip_prefix("root//").unwrap_or(package);
    format!("{package}:{}", label["name"].as_str().unwrap_or("?"))
}

/// A step of the critical path, such as `rustc metadata [pic] third-party/rust:quote-1.0.45`, or the kind of step
/// for one that is not an action (`Waiting`, `Analysis`).
fn describe(entry: &Value) -> String {
    let Some((kind, value)) = entry.as_object().and_then(|entry| entry.iter().next()) else {
        return "?".to_owned();
    };
    if kind != "ActionExecution" {
        return kind.clone();
    }
    let mut what = value["name"]["category"].as_str().unwrap_or("?").to_owned();
    if let Some(identifier) = value["name"]["identifier"]
        .as_str()
        .filter(|identifier| !identifier.is_empty())
    {
        let _ = write!(what, " {identifier}");
    }
    let _ = write!(what, " {}", target(&value["owner"]));
    what
}

/// The build server ./scripts/buck configured buck2 for (crates/be-tool-bootstrap's server.rs writes it), as the
/// record's address and the header that authorizes it, when it is a named server behind Caddy rather than this
/// machine or a custom address.
fn destination(repository: &Path) -> Option<(String, Option<String>)> {
    let config = std::fs::read_to_string(repository.join("target/build-server.buckconfig")).ok()?;
    let server = config
        .lines()
        .find_map(|line| line.strip_prefix("# The server: "))?
        .trim();
    if server.is_empty() || server.contains(':') {
        return None;
    }
    let header = config
        .lines()
        .find_map(|line| line.strip_prefix("http_headers = "))
        .map(|header| header.trim().replacen(':', ": ", 1));
    Some((format!("https://{server}/__be3-stats/build"), header))
}

/// Sends the record to the build server with curl, printing why when it could not. A cloud session's proxy adds the
/// token when buck2's config has none.
pub fn send(repository: &Path, record: &Value) {
    let Some((url, header)) = destination(repository) else {
        return;
    };
    let path = repository.join("target/verify-record.json");
    if let Err(error) = std::fs::write(&path, record.to_string()) {
        println!("Could not write {}: {error}", path.display());
        return;
    }
    // curl's config on its standard input, so the token is on no command line.
    let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let mut config = format!(
        "url = \"{url}\"\nheader = \"Content-Type: application/json\"\ndata-binary = \"@{}\"\nmax-time = 20\nsilent\nshow-error\nfail\noutput = \"{}\"\n",
        quote(&path.to_string_lossy()),
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    );
    if let Some(header) = header {
        let _ = writeln!(config, "header = \"{}\"", quote(&header));
    }
    let child = Command::new("curl")
        .args(["--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();
    let result = child.and_then(|mut child| {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes())?;
        }
        child.wait_with_output()
    });
    match result {
        Ok(output) if output.status.success() => {}
        Ok(output) => println!(
            "Could not send this build's record to the dashboard: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => {
            println!("Could not send this build's record to the dashboard: curl: {error}")
        }
    }
}
