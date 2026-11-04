//! Notices a buck2 daemon that has stopped. buck2's client writes the command's
//! event log as it goes, and even while every action waits on the build server
//! it adds a snapshot every few seconds, so a log that has not grown for
//! minutes means the daemon has stopped making progress, which it does not
//! recover from by itself (guides/build_server.md, "When a build keeps
//! failing"). The watchdog then saves what the daemon and the relays were
//! waiting on, kills the daemon, and the command counts as an infrastructure
//! error, which `buck::retrying` runs again on a fresh daemon.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::{repository, server};

/// How long a log may stay the same size. A healthy one grows every 15 to 20
/// seconds while the build only waits.
const STOPPED_AFTER: Duration = Duration::from_secs(5 * 60);
const POLL: Duration = Duration::from_secs(10);
const CLIENT_GRACE: Duration = Duration::from_secs(60);

pub struct Watchdog {
    stop: Sender<()>,
    thread: JoinHandle<bool>,
    build_id: PathBuf,
}

/// Watches the command whose build id buck2 writes to `build_id`, the path
/// given as its `--write-build-id`, run by the buck2 client `client`.
pub fn watch(buck2: &Path, build_id: PathBuf, client: u32) -> Watchdog {
    let (stop, stopped) = mpsc::channel();
    let buck2 = buck2.to_owned();
    let file = build_id.clone();
    let thread = std::thread::spawn(move || {
        let mut size = None;
        let mut changed = Instant::now();
        loop {
            match stopped.recv_timeout(POLL) {
                Err(RecvTimeoutError::Timeout) => {}
                _ => return false,
            }
            let Some(log) = event_log(&file) else {
                changed = Instant::now();
                continue;
            };
            let now = std::fs::metadata(&log).map(|metadata| metadata.len()).ok();
            if now != size {
                size = now;
                changed = Instant::now();
                continue;
            }
            if changed.elapsed() >= STOPPED_AFTER {
                restart(&buck2, &log);
                // The client normally exits once its daemon is gone; one that
                // is stuck itself is stopped too.
                if let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(CLIENT_GRACE) {
                    stop_client(client);
                }
                return true;
            }
        }
    });
    Watchdog {
        stop,
        thread,
        build_id,
    }
}

impl Watchdog {
    /// Stops watching once buck2 has exited, and says whether the daemon was
    /// found stopped and killed.
    pub fn finish(self) -> bool {
        let _ = self.stop.send(());
        let restarted = self.thread.join().unwrap_or(false);
        let _ = std::fs::remove_file(&self.build_id);
        restarted
    }
}

fn event_log(build_id: &Path) -> Option<PathBuf> {
    let id = std::fs::read_to_string(build_id).ok()?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let suffix = format!("_{id}_events.pb.zst");
    std::fs::read_dir(repository().join("buck-out/v2/log"))
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(&suffix))
        })
}

fn restart(buck2: &Path, log: &Path) {
    let saved = repository().join("target/buck2-stopped.txt");
    let mut report = format!(
        "buck2's event log {} had not grown for {} minutes.\n\n",
        log.display(),
        STOPPED_AFTER.as_secs() / 60
    );
    let status = output_within(Command::new(buck2).arg("status"), 30);
    report.push_str("== buck2 status\n");
    report.push_str(&status);
    for address in server::relay::addresses() {
        if let Some(calls) = relay_calls(&address) {
            report.push_str(&format!("\n== the relay on {address}\n{calls}"));
        }
    }
    if cfg!(target_os = "linux")
        && let Some(pid) = daemon_pid()
    {
        let mut gdb = Command::new("gdb");
        gdb.args(["-p", &pid, "-batch", "-ex", "thread apply all bt"]);
        report.push_str("\n== the daemon's threads (gdb)\n");
        report.push_str(&output_within(&mut gdb, 120));
    }
    let _ = std::fs::write(&saved, report);
    eprintln!(
        "buck2's daemon has made no progress for {} minutes, so it is being restarted. What it \
         and the relays were waiting on is in {} (guides/build_server.md).",
        STOPPED_AFTER.as_secs() / 60,
        saved.display()
    );
    let _ = Command::new(buck2)
        .arg("kill")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn stop_client(pid: u32) {
    let pid = pid.to_string();
    let mut command = if cfg!(windows) {
        let mut command = Command::new("taskkill");
        command.args(["/F", "/PID", &pid]);
        command
    } else {
        let mut command = Command::new("kill");
        command.args(["-9", &pid]);
        command
    };
    let _ = command.stdout(Stdio::null()).stderr(Stdio::null()).status();
}

/// The relay's list of the calls it has in flight and its goroutines, when
/// a relay listens there.
fn relay_calls(address: &str) -> Option<String> {
    let address = address.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    write!(
        stream,
        "GET /__be3-re-relay/calls HTTP/1.0\r\nHost: {address}\r\n\r\n"
    )
    .ok()?;
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    let answer = String::from_utf8_lossy(&answer).into_owned();
    let (head, body) = answer.split_once("\r\n\r\n")?;
    head.starts_with("HTTP/1.0 200")
        .then(|| body.to_owned())
        .or_else(|| head.starts_with("HTTP/1.1 200").then(|| body.to_owned()))
}

/// The daemon's process id, from the file its clients find it by, which is
/// there even when it answers nothing.
fn daemon_pid() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let checkout = repository().strip_prefix("/").ok()?;
    let info = Path::new(&home)
        .join(".buck/buckd")
        .join(checkout)
        .join("v2/buckd.info");
    let info = std::fs::read_to_string(info).ok()?;
    let rest = &info[info.find("\"pid\":")? + "\"pid\":".len()..];
    let pid: String = rest
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!pid.is_empty()).then_some(pid)
}

/// What a command prints, or why it printed nothing, given at most `seconds`.
fn output_within(command: &mut Command, seconds: u64) -> String {
    let mut child = match command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return format!("could not run it: {error}\n"),
    };
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(stdout) = stdout.as_mut() {
            let _ = stdout.read_to_end(&mut text);
        }
        if let Some(stderr) = stderr.as_mut() {
            let _ = stderr.read_to_end(&mut text);
        }
        text
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let text = reader.join().unwrap_or_default();
                return format!(
                    "{}(no answer in {seconds} seconds)\n",
                    String::from_utf8_lossy(&text)
                );
            }
        }
    }
    String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned()
}
