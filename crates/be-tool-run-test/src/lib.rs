//! What starts every test (be_test in buck/dev/test.bzl), the same under
//! `buck2 test`, as ./scripts/verify's action and as ./scripts/test's:
//!
//! ```text
//! run-test [--library-path PATHS] [--time-limit 20m] [--one-test]
//!          [--paintings OUT] [--passed OUT] [--record OUT] -- COMMAND...
//! ```
//!
//! - TMPDIR is a fresh directory under /tmp, removed afterwards. A worker's own
//!   temporary directory can sit so deep that a Unix socket made in it is
//!   longer than a socket's path may be (SUN_LEN), which failed every test that
//!   listens on one (a Wayland server, a D-Bus daemon, an OpenXR runtime).
//! - `--library-path` is the test's LD_LIBRARY_PATH, set for the test alone:
//!   Namespace's workers replace an action's own LD_LIBRARY_PATH with theirs,
//!   and a sysroot's libc in it would load into the launcher.
//! - `--time-limit` stops the test, and everything it started, once it has run
//!   that long (`90s`, `20m`, `1h`). A test that hangs would otherwise hold its
//!   worker until the build server gives up on it, and every later run of the
//!   same action would wait on that one.
//! - `--one-test` fails a libtest harness that passed without running exactly
//!   one test (its `running 1 test` line): a run_apart part, which names its
//!   file's test with --exact, so that a file whose test is not named after it
//!   cannot pass having run nothing.
//! - `--paintings OUT` makes OUT/used and OUT/changed and names them in
//!   USED_PAINTINGS and CHANGED_PAINTINGS, where a painting test writes what
//!   it compared and what it accepted (guides/testing_a_gui.md).
//! - `--passed OUT` writes an empty OUT when the test passes.
//! - `--record OUT` writes the test's output to OUT/log and its exit status to
//!   OUT/status, names OUT/changed in CHANGED_PAINTINGS, and exits 0, so a
//!   failure is kept like a pass (./scripts/test, buck/dev/test.bxl).
//!
//! Otherwise it exits with the test's status, 128 and the signal for a test a
//! signal stopped, and 124 for one the time limit stopped.

use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::Duration;

pub const USAGE: &str = "usage: run-test [--library-path PATHS] [--time-limit 20m] [--one-test] [--paintings OUT] [--passed OUT] [--record OUT] -- COMMAND...";

/// What libtest prints first when it runs exactly one test.
const ONE_TEST: &str = "running 1 test";

/// The status a test the time limit stopped exits with, as timeout(1)'s.
pub const TIMED_OUT: i32 = 124;

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub library_path: Option<OsString>,
    /// The limit as it was written, for the message, and as a duration.
    pub time_limit: Option<(String, Duration)>,
    pub one_test: bool,
    pub paintings: Option<PathBuf>,
    pub passed: Option<PathBuf>,
    pub record: Option<PathBuf>,
    pub command: Vec<OsString>,
}

pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            options.command = arguments.collect();
            break;
        }
        let flag = argument.to_string_lossy().into_owned();
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--library-path" => options.library_path = Some(value()?),
            "--time-limit" => {
                let text = value()?.to_string_lossy().into_owned();
                let limit = parse_duration(&text)?;
                options.time_limit = Some((text, limit));
            }
            "--one-test" => options.one_test = true,
            "--paintings" => options.paintings = Some(value()?.into()),
            "--passed" => options.passed = Some(value()?.into()),
            "--record" => options.record = Some(value()?.into()),
            _ => return Err(format!("unknown argument {flag}\n{USAGE}")),
        }
    }
    if options.command.is_empty() {
        return Err(format!("no command to run\n{USAGE}"));
    }
    Ok(options)
}

/// A number of seconds, or one followed by s, m or h.
pub fn parse_duration(text: &str) -> Result<Duration, String> {
    let (number, unit) = match text.char_indices().last() {
        Some((at, unit @ ('s' | 'm' | 'h'))) => (&text[..at], unit),
        _ => (text, 's'),
    };
    let number: u64 = number.parse().map_err(|_| {
        format!("a time limit is a number of seconds, or one followed by s, m or h, not {text:?}")
    })?;
    let seconds = match unit {
        'h' => number * 3600,
        'm' => number * 60,
        _ => number,
    };
    Ok(Duration::from_secs(seconds))
}

/// Runs the test as the options say, and returns what to exit with.
pub fn run(options: &Options) -> Result<i32, String> {
    let temporary = TemporaryDirectory::new()?;
    let mut command = Command::new(&options.command[0]);
    command
        .args(&options.command[1..])
        .env("TMPDIR", &temporary.0);
    if let Some(library_path) = &options.library_path {
        command.env("LD_LIBRARY_PATH", library_path);
    }
    if let Some(out) = &options.paintings {
        let (used, changed) = (out.join("used"), out.join("changed"));
        create_dir(&used)?;
        create_dir(&changed)?;
        command
            .env("USED_PAINTINGS", used)
            .env("CHANGED_PAINTINGS", changed);
    }
    let mut log = None;
    if let Some(out) = &options.record {
        let changed = out.join("changed");
        create_dir(&changed)?;
        command.env("CHANGED_PAINTINGS", changed);
        let file = File::create(out.join("log"))
            .map_err(|error| format!("cannot write {}: {error}", out.join("log").display()))?;
        let stderr = file.try_clone().map_err(|error| error.to_string())?;
        command.stdout(file).stderr(stderr);
        log = Some(out.join("log"));
    } else if options.one_test {
        command.stdout(Stdio::piped());
    }
    let mut ran_one = None;
    let mut status = match run_command(command, options.time_limit.as_ref()) {
        Ok((status, copied)) => {
            ran_one = copied;
            status
        }
        Err(message) => {
            // As a shell says it, and exits 127.
            report(log.as_deref(), &message);
            127
        }
    };
    if status == TIMED_OUT
        && let Some((limit, _)) = &options.time_limit
    {
        report(
            log.as_deref(),
            &format!(
                "the test ran for over {limit} and was stopped: one of its tests hangs, the one said above to have been running for over 60 seconds"
            ),
        );
    }
    if options.one_test && status == 0 {
        let ran_one = match &log {
            Some(log) => std::fs::read_to_string(log)
                .is_ok_and(|text| text.lines().any(|line| line == ONE_TEST)),
            None => ran_one.unwrap_or(false),
        };
        if !ran_one {
            report(
                log.as_deref(),
                "the test passed without running exactly one test: a run_apart part runs the test named after its file (buck/cargo/defs.bzl), so the file needs one test of that name",
            );
            status = 1;
        }
    }
    drop(temporary);
    if let Some(out) = &options.record {
        let path = out.join("status");
        std::fs::write(&path, format!("{status}\n"))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        return Ok(0);
    }
    if status == 0
        && let Some(passed) = &options.passed
    {
        File::create(passed)
            .map_err(|error| format!("cannot write {}: {error}", passed.display()))?;
    }
    Ok(status)
}

fn create_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))
}

/// A message of the launcher's own, after the test's output: in the log when
/// there is one, else on stderr.
fn report(log: Option<&Path>, message: &str) {
    let appended = log.is_some_and(|log| {
        std::fs::OpenOptions::new()
            .append(true)
            .open(log)
            .and_then(|mut file| writeln!(file, "{message}"))
            .is_ok()
    });
    if !appended {
        eprintln!("{message}");
    }
}

/// Runs the command to its end or its time limit. Its status, and, when its stdout was piped,
/// whether that said it ran one test, copied to the launcher's own stdout as it came.
fn run_command(
    mut command: Command,
    time_limit: Option<&(String, Duration)>,
) -> Result<(i32, Option<bool>), String> {
    command.stdin(Stdio::null());
    process::own_group(&mut command);
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    let id = child.id();
    process::stop_with_launcher(id);
    let copier = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let mut ran_one = false;
            let mut out = std::io::stdout();
            for line in std::io::BufReader::new(stdout)
                .split(b'\n')
                .map_while(Result::ok)
            {
                ran_one |= line == ONE_TEST.as_bytes();
                let _ = out.write_all(&line);
                let _ = out.write_all(b"\n");
                let _ = out.flush();
            }
            ran_one
        })
    });
    let status = wait_within(child, id, time_limit)?;
    let ran_one = copier.map(|copier| copier.join().unwrap_or(false));
    Ok((status, ran_one))
}

fn wait_within(
    child: Child,
    id: u32,
    time_limit: Option<&(String, Duration)>,
) -> Result<i32, String> {
    let Some((_, limit)) = time_limit else {
        return wait(child);
    };
    let (sender, receiver) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let status = wait(child);
        let _ = sender.send(());
        status
    });
    let timed_out = receiver.recv_timeout(*limit).is_err();
    if timed_out {
        process::kill_group(id);
    }
    let status = waiter
        .join()
        .map_err(|_| "the test's waiter panicked".to_owned())??;
    Ok(if timed_out { TIMED_OUT } else { status })
}

fn wait(mut child: Child) -> Result<i32, String> {
    let status = child.wait().map_err(|error| error.to_string())?;
    Ok(code(status))
}

fn code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

/// Has a signal that stops the launcher, as buck2 stops an action it
/// cancels, stop the test it runs too, which is in a group of its own. For the
/// launcher's main alone: it replaces the process's handlers.
pub fn forward_signals() {
    process::forward_signals();
}

/// A new directory under /tmp, removed when dropped.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Result<Self, String> {
        let base = if cfg!(unix) {
            PathBuf::from("/tmp")
        } else {
            std::env::temp_dir()
        };
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or_default()
            ^ (u128::from(std::process::id()) << 64);
        for attempt in 0..1000u128 {
            let mixed = (seed.wrapping_add(attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15))) as u64;
            let path = base.join(format!("test.{:06x}", mixed % 0x100_0000));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(format!("cannot create {}: {error}", path.display())),
            }
        }
        Err(format!(
            "cannot find a free directory name under {}",
            base.display()
        ))
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The test in a process group of its own, as timeout(1) runs it, so that
/// stopping it stops what it started too.
#[cfg(unix)]
mod process {
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use std::sync::atomic::{AtomicI32, Ordering};

    const SIGHUP: i32 = 1;
    const SIGINT: i32 = 2;
    const SIGKILL: i32 = 9;
    const SIGTERM: i32 = 15;

    // unsafe(ffi)
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
        fn signal(signal: i32, handler: extern "C" fn(i32)) -> usize;
        fn _exit(status: i32) -> !;
    }

    static GROUP: AtomicI32 = AtomicI32::new(0);

    pub fn own_group(command: &mut Command) {
        command.process_group(0);
    }

    pub fn kill_group(id: u32) {
        // unsafe(ffi)
        unsafe {
            kill(-(id as i32), SIGKILL);
        }
    }

    extern "C" fn stop(received: i32) {
        let group = GROUP.load(Ordering::SeqCst);
        // unsafe(ffi)
        unsafe {
            if group > 0 {
                kill(-group, SIGKILL);
            }
            _exit(128 + received);
        }
    }

    pub fn stop_with_launcher(id: u32) {
        GROUP.store(id as i32, Ordering::SeqCst);
    }

    pub fn forward_signals() {
        for received in [SIGHUP, SIGINT, SIGTERM] {
            // unsafe(ffi)
            unsafe {
                signal(received, stop);
            }
        }
    }
}

#[cfg(not(unix))]
mod process {
    use std::process::Command;

    pub fn own_group(_command: &mut Command) {}

    pub fn kill_group(id: u32) {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &id.to_string()])
            .status();
    }

    pub fn stop_with_launcher(_id: u32) {}

    pub fn forward_signals() {}
}

#[cfg(test)]
mod tests;
