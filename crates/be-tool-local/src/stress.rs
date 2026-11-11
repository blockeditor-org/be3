use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

const USAGE: &str = "usage: local-tools stress NAME [--repeat N] [--parallel K] [--limit SECONDS]";
const HARNESS: &str = "//crates/be-host-tests:test-harness";
const BUNDLE: &str = "//crates/be-host-tests:host-with-all";
const HOST: &str = "//crates/be-host:bin";
const AMD64: &str = "//buck/sysroot:amd64";
const AMD64_TEST: &str = "//buck/sysroot:amd64-test";
const LAVAPIPE: &str = "//buck/sysroot:lavapipe_icd.json";
const REPEAT: usize = 20;

/// A run that takes longer than this is killed and counted as timed out: a hung host would
/// otherwise stop the whole stress run. Every step of a host test gives up after 30 seconds, so
/// a run of a few steps ends well before this when nothing hangs.
const LIMIT: Duration = Duration::from_secs(300);

/// Where a failing run's output goes, one file each, under the name stressed.
const OUTPUT: &str = "target/stress";

/// The environment variables `run_env` in crates/be-host-tests/BUCK sets for the harness, and
/// `CARGO_MANIFEST_DIR`. Nothing checks the two lists agree (a test cannot read that BUCK file
/// from its sandbox): a variable added there is added here, and a harness that fails at once
/// with a missing variable says so.
const ENVIRONMENT: [&str; 6] = [
    "BE_BUNDLE",
    "BE_HOST",
    "HOST_LIBRARIES",
    "VK_ICD_FILENAMES",
    "XKB_CONFIG_ROOT",
    "CARGO_MANIFEST_DIR",
];

/// Runs the host tests whose names contain NAME again and again, several at once, and says how
/// many failed and why: a flake that shows only under load shows here (guides/testing_a_gui.md,
/// 6, Debugging). `./scripts/test` caches a passing run, so running it again proves
/// nothing.
///
/// Every run uses the one host that has every app (`:host-with-all`), where `./scripts/test`
/// stages the apps a test names in `APPS` in crates/be-host-tests/BUCK: a test that fails only
/// for want of an app, or only with all of them, differs between the two.
pub fn main(arguments: Vec<String>) -> ExitCode {
    match run(arguments) {
        Ok(failed) => match failed {
            0 => ExitCode::SUCCESS,
            _ => ExitCode::FAILURE,
        },
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, PartialEq)]
struct Options {
    name: String,
    repeat: usize,
    parallel: usize,
    limit: Duration,
}

/// Twice the cores: enough to slow every step down, short of the load at which single steps
/// pass the harness's 30 seconds with nothing wrong.
fn default_parallel() -> usize {
    thread::available_parallelism().map_or(4, |cores| cores.get() * 2)
}

fn options(arguments: Vec<String>, parallel: usize) -> Result<Options, String> {
    let mut name = None;
    let mut repeat = REPEAT;
    let mut parallel = parallel;
    let mut limit = LIMIT;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let mut number = |what: &str| -> Result<usize, String> {
            arguments
                .next()
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{what} takes a number above 0\n{USAGE}"))
        };
        match argument.as_str() {
            "--repeat" => repeat = number("--repeat")?,
            "--parallel" => parallel = number("--parallel")?,
            "--limit" => limit = Duration::from_secs(number("--limit")? as u64),
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}\n{USAGE}")),
            _ if name.is_some() => return Err(USAGE.to_owned()),
            _ => name = Some(argument),
        }
    }
    Ok(Options {
        name: name.ok_or_else(|| USAGE.to_owned())?,
        repeat,
        parallel,
        limit,
    })
}

/// What went wrong in one run, which failures are grouped by.
struct Failure {
    reason: String,
    log: PathBuf,
}

fn run(arguments: Vec<String>) -> Result<usize, String> {
    let options = options(arguments, default_parallel())?;
    let built = build()?;
    let output = PathBuf::from(OUTPUT).join(file_name(&options.name));
    let _ = fs::remove_dir_all(&output);
    fs::create_dir_all(&output)
        .map_err(|error| format!("could not make {}: {error}", output.display()))?;
    let next = AtomicUsize::new(0);
    let failures: Mutex<Vec<Failure>> = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..options.parallel.min(options.repeat) {
            scope.spawn(|| {
                loop {
                    let run = next.fetch_add(1, Ordering::SeqCst);
                    if run >= options.repeat {
                        return;
                    }
                    let log = output.join(format!("{run}.log"));
                    if let Err(reason) = once(&built, &options.name, options.limit, &log) {
                        failures
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(Failure { reason, log });
                    }
                }
            });
        }
    });
    let failures = failures
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    println!(
        "{}: {} of {} runs failed, {} at a time",
        options.name,
        failures.len(),
        options.repeat,
        options.parallel
    );
    print!("{}", report(&failures));
    if !failures.is_empty() {
        println!("The failing runs' output is in {}/.", output.display());
    }
    Ok(failures.len())
}

/// Failures grouped by their reason, with the first three logs of each.
fn report(failures: &[Failure]) -> String {
    let mut groups: BTreeMap<String, Vec<&Path>> = BTreeMap::new();
    for failure in failures {
        groups
            .entry(normalise(&failure.reason))
            .or_default()
            .push(&failure.log);
    }
    let mut text = String::new();
    for (reason, logs) in groups {
        let names: Vec<String> = logs
            .iter()
            .take(3)
            .filter_map(|log| log.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        text.push_str(&format!(
            "  {} x {reason} ({}{})\n",
            logs.len(),
            names.join(", "),
            if logs.len() > names.len() { ", ..." } else { "" }
        ));
    }
    text
}

/// A reason with its digits replaced, so failures that differ only in a thread id, a size or a
/// count are one group.
fn normalise(reason: &str) -> String {
    let mut normalised = String::new();
    let mut digits = false;
    for character in reason.chars() {
        match character.is_ascii_digit() {
            true if !digits => {
                digits = true;
                normalised.push('N');
            }
            true => {}
            false => {
                digits = false;
                normalised.push(character);
            }
        }
    }
    normalised
}

/// A name that is one path component.
fn file_name(name: &str) -> String {
    name.chars()
        .map(|character| match character.is_ascii_alphanumeric() || character == '_' {
            true => character,
            false => '_',
        })
        .collect()
}

struct Built {
    harness: PathBuf,
    environment: Vec<(&'static str, String)>,
}

/// What `run_env` in crates/be-host-tests/BUCK sets for the harness, built here.
fn build() -> Result<Built, String> {
    let targets = [HARNESS, BUNDLE, HOST, AMD64_TEST, AMD64, LAVAPIPE];
    let output = Command::new("./scripts/buck")
        .args(["build", "--show-full-output"])
        .args(targets)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("could not run ./scripts/buck: {error}"))?;
    if !output.status.success() {
        return Err("Building the host tests failed.".to_owned());
    }
    let listed = String::from_utf8_lossy(&output.stdout);
    let path = |target: &str| -> Result<PathBuf, String> {
        let wanted = format!("root{target}");
        listed
            .lines()
            .find_map(|line| {
                let (name, path) = line.split_once(' ')?;
                (name == wanted).then(|| PathBuf::from(path.trim()))
            })
            .ok_or_else(|| format!("buck did not say where it built {target}"))
    };
    let amd64_test = path(AMD64_TEST)?;
    let amd64 = path(AMD64)?;
    let libraries = |sysroot: &Path| sysroot.join("usr/lib/x86_64-linux-gnu");
    let [bundle, host, libraries_variable, icd, xkb, manifest] = ENVIRONMENT;
    Ok(Built {
        harness: path(HARNESS)?,
        environment: vec![
            (bundle, path(BUNDLE)?.display().to_string()),
            (host, path(HOST)?.display().to_string()),
            (
                libraries_variable,
                format!(
                    "{}:{}",
                    libraries(&amd64_test).display(),
                    libraries(&amd64).display()
                ),
            ),
            (icd, path(LAVAPIPE)?.display().to_string()),
            (
                xkb,
                amd64_test.join("usr/share/X11/xkb").display().to_string(),
            ),
            (
                manifest,
                env::current_dir()
                    .map_err(|error| format!("no working directory: {error}"))?
                    .join("crates/be-host-tests")
                    .display()
                    .to_string(),
            ),
        ],
    })
}

/// One run of the tests named, which fails with what went wrong: the panic's message, how the
/// harness ended when it did not panic, or that it ran past `limit` and was killed.
fn once(built: &Built, name: &str, limit: Duration, log: &Path) -> Result<(), String> {
    let mut child = Command::new(&built.harness)
        .arg(name)
        .envs(built.environment.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run the harness: {error}"))?;
    let readers = [
        child.stdout.take().map(reader),
        child.stderr.take().map(reader),
    ];
    let (status, timed_out) = wait(&mut child, limit);
    let text: String = readers
        .into_iter()
        .flatten()
        .map(|reader| reader.join().unwrap_or_default())
        .collect();
    if let (Some(status), false) = (status, timed_out)
        && status.success()
    {
        return Ok(());
    }
    let _ = fs::write(log, &text);
    Err(match (timed_out, status) {
        (true, _) => format!("timed out: the run took more than {} seconds", limit.as_secs()),
        (false, Some(status)) => reason(&text, &ended(status)),
        (false, None) => reason(&text, "the harness could not be waited for"),
    })
}

fn reader(mut pipe: impl std::io::Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Waits for the child until `limit`, then kills it: how it ended and whether it was killed.
fn wait(child: &mut Child, limit: Duration) -> (Option<std::process::ExitStatus>, bool) {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (Some(status), false),
            Ok(None) if started.elapsed() >= limit => {
                let _ = child.kill();
                return (child.wait().ok(), true);
            }
            Ok(None) => thread::park_timeout(Duration::from_millis(50)),
            Err(_) => return (None, false),
        }
    }
}

/// How a process that did not succeed ended, naming the signal of one a signal killed.
fn ended(status: std::process::ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(signal) = status.signal() {
            return format!("the harness was killed by signal {signal}");
        }
    }
    format!("the harness exited with {status}")
}

/// The panic's own message, which tells failures apart, or `ended` when there was none.
fn reason(text: &str, ended: &str) -> String {
    let mut lines = text.lines();
    lines
        .find(|line| line.contains(" panicked at "))
        .and_then(|_| lines.next())
        .map_or_else(|| ended.to_owned(), |message| message.trim().to_owned())
}

#[cfg(test)]
mod tests;
