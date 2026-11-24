use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::{env, fs, thread};

const USAGE: &str =
    "usage: be-tool-local stress NAME [--repeat N] [--parallel K] [--limit SECONDS] [--apart] [--latency MS]";
/// The host tests' package, whose `:test` runs every test against the host that has every app
/// and whose `:test-FILE` runs one against the host that stages the apps it names
/// (crates/be-host-tests/BUCK).
const PACKAGE: &str = "//crates/be-host-tests";
/// The host tests' sources: each file in a `tests/` directory under them is a test of its own,
/// named after the file (run_apart in buck/cargo/defs.bzl).
const SOURCES: &str = "crates/be-host-tests/src";
const REPEAT: usize = 20;

/// A run that takes longer than this is stopped, with everything it started, and counted as
/// timed out: a hung host would otherwise stop the whole stress run. Every step of a host test
/// gives up after 30 seconds, so a run of a few steps ends well before this when nothing hangs.
const LIMIT: Duration = Duration::from_secs(300);

/// Where a failing run's output goes, one file each, under the name stressed.
const OUTPUT: &str = "target/stress";

/// Runs the host tests whose names contain NAME again and again, several at once, and says how
/// many failed and why: a flake that shows only under load shows here (guides/testing_a_gui.md,
/// 6, Debugging), a painting that differs from the accepted one included. `./scripts/test`
/// caches a passing run, so running it again proves nothing.
///
/// Each run is the test's own command as buck/dev/test.bzl describes it (its `[command]`):
/// crates/be-tool-run-test, which gives it a TMPDIR of its own and stops it and all it started
/// at the time limit, with the test's environment. Its paintings are compared as verify
/// compares them, by be-tool-paint-compare. Every run uses `:test`, the host that has every
/// app, unless `--apart`: then each test runs as its own part, staging only the apps it names
/// in `APPS` in crates/be-host-tests/BUCK, as `./scripts/test` and verify do, so a flake that
/// needs the smaller host's timing shows too.
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
    /// Each test runs as its own part, against the host that stages the apps it names.
    apart: bool,
    /// How long the server takes to answer a device of a test's `Server`, through its relay.
    latency: Option<Duration>,
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
    let mut apart = false;
    let mut latency = None;
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
            "--latency" => {
                latency = Some(Duration::from_millis(number("--latency")? as u64));
            }
            "--apart" => apart = true,
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
        apart,
        latency,
    })
}

/// What went wrong in one run, which failures are grouped by.
struct Failure {
    reason: String,
    log: PathBuf,
}

fn run(arguments: Vec<String>) -> Result<usize, String> {
    let options = options(arguments, default_parallel())?;
    let tests = tests_named(&test_files(Path::new(SOURCES))?, &options.name);
    if tests.is_empty() {
        return Err(format!(
            "No host test's name contains {}: stress runs only the host tests (the files in a tests/ directory under {SOURCES}), and matches NAME against each one's full name, such as tests::STEM::STEM.",
            options.name
        ));
    }
    let jobs = build(&options, &tests)?;
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
                    for (index, job) in jobs.iter().enumerate() {
                        let name = match jobs.len() {
                            1 => run.to_string(),
                            _ => format!("{run}-{index}"),
                        };
                        let log = output.join(format!("{name}.log"));
                        if let Err(reason) = once(job, &options, &log, &output.join(&name)) {
                            let reason = match &job.test {
                                Some(test) if jobs.len() > 1 => format!("{test}: {reason}"),
                                _ => reason,
                            };
                            failures
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .push(Failure { reason, log });
                        }
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
        options.repeat * jobs.len(),
        options.parallel
    );
    print!("{}", report(&failures));
    if !failures.is_empty() {
        println!(
            "The failing runs' output is in {}/, as N.log, the whole logs of the programs each drove in N.logs/, and the paintings of run N that differ from the accepted ones in N/changed/.",
            output.display()
        );
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
            if logs.len() > names.len() {
                ", ..."
            } else {
                ""
            }
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
        .map(
            |character| match character.is_ascii_alphanumeric() || character == '_' {
                true => character,
                false => '_',
            },
        )
        .collect()
}

/// A host test: its part's name, as `:test-PART` (`--apart`), and its full name, as libtest's
/// filter reads it.
#[derive(Clone, Debug, PartialEq)]
struct Test {
    part: String,
    libtest: String,
}

/// The tests under `sources` as cargo_test names them (buck/cargo/defs.bzl): a file
/// `src/MODULE/tests/STEM.rs` is the part `MODULE-tests-STEM` (`tests/STEM.rs` is `STEM`) and
/// runs `MODULE::tests::STEM::STEM`.
fn test_files(sources: &Path) -> Result<Vec<Test>, String> {
    fn walk(directory: &Path, module: &mut Vec<String>, tests: &mut Vec<Test>) -> Result<(), String> {
        let entries = fs::read_dir(directory)
            .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                module.push(name);
                walk(&path, module, tests)?;
                module.pop();
            } else if module.last().is_some_and(|last| last == "tests")
                && let Some(stem) = name.strip_suffix(".rs")
            {
                let mut path = module.clone();
                path.push(stem.to_owned());
                let joined = path.join("/");
                tests.push(Test {
                    part: joined.strip_prefix("tests/").unwrap_or(&joined).replace('/', "-"),
                    libtest: format!("{}::{stem}", path.join("::")),
                });
            }
        }
        Ok(())
    }
    let mut tests = Vec::new();
    walk(sources, &mut Vec::new(), &mut tests)?;
    tests.sort_by(|a, b| a.part.cmp(&b.part));
    Ok(tests)
}

/// The tests whose full names contain `name`, as libtest's filter matches them.
fn tests_named(tests: &[Test], name: &str) -> Vec<Test> {
    tests
        .iter()
        .filter(|test| test.libtest.contains(name))
        .cloned()
        .collect()
}

/// One test command to run again and again: a test's `[command]` (buck/dev/test.bzl), and the
/// filter that follows it.
#[derive(Debug, PartialEq)]
struct Job {
    /// The test a part runs, for `--apart`; `None` for the whole harness, filtered.
    test: Option<String>,
    launcher: Vec<String>,
    command: Vec<String>,
    environment: Vec<(String, String)>,
}

impl Job {
    /// A `[command]`'s JSON: `launcher` and `command`, lists of words, and `env`, each value a
    /// list of words that buck2 would join with spaces.
    fn parse(test: Option<String>, text: &str) -> Result<Job, String> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|error| format!("a test's [command] is not JSON: {error}"))?;
        let words = |value: &serde_json::Value| -> Option<Vec<String>> {
            value
                .as_array()?
                .iter()
                .map(|word| word.as_str().map(str::to_owned))
                .collect()
        };
        let field = |key: &str| {
            words(&value[key]).ok_or_else(|| format!("a test's [command] has no list {key}"))
        };
        let environment = value["env"]
            .as_object()
            .ok_or("a test's [command] has no env")?
            .iter()
            .map(|(key, value)| {
                words(value)
                    .map(|words| (key.clone(), words.join(" ")))
                    .ok_or_else(|| format!("a test's [command] has no words for {key}"))
            })
            .collect::<Result<_, String>>()?;
        Ok(Job {
            test,
            launcher: field("launcher")?,
            command: field("command")?,
            environment,
        })
    }

    /// Where the test's accepted paintings are: its crate's snapshots/.
    fn accepted(&self) -> PathBuf {
        self.environment
            .iter()
            .find(|(key, _)| key == "CARGO_MANIFEST_DIR")
            .map(|(_, manifest)| Path::new(manifest).join("snapshots"))
            .unwrap_or_default()
    }
}

/// The `[command]` of `:test`, or with `--apart` of each test's `:test-STEM`, built in one
/// buck2 invocation with everything they run.
fn build(options: &Options, tests: &[Test]) -> Result<Vec<Job>, String> {
    let targets: Vec<(Option<String>, String)> = match options.apart {
        false => vec![(None, format!("{PACKAGE}:test[command]"))],
        true => tests
            .iter()
            .map(|test| {
                (
                    Some(test.part.clone()),
                    format!("{PACKAGE}:test-{}[command]", test.part),
                )
            })
            .collect(),
    };
    let output = Command::new("./scripts/buck")
        .args(["build", "--show-full-output"])
        .args(targets.iter().map(|(_, target)| target))
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("could not run ./scripts/buck: {error}"))?;
    if !output.status.success() {
        return Err("Building the host tests failed.".to_owned());
    }
    let listed = String::from_utf8_lossy(&output.stdout);
    targets
        .into_iter()
        .map(|(test, target)| {
            let wanted = format!("root{target}");
            let path = listed
                .lines()
                .find_map(|line| {
                    let (name, path) = line.split_once(' ')?;
                    (name == wanted).then(|| PathBuf::from(path.trim()))
                })
                .ok_or_else(|| format!("buck did not say where it built {target}"))?;
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            Job::parse(test, &text)
        })
        .collect()
}

/// One run of the job, which fails with what went wrong: the panic's message, how the harness
/// ended when it did not panic, that it ran past `limit` and was stopped, or that a painting
/// differs from the accepted one. run-test records the run in `paintings`.run (its `--record`:
/// the log, the status and the paintings made, which the test compares none of), and
/// be-tool-paint-compare compares those with the accepted ones into `paintings`, so that every
/// run keeps its own: those that differ stay in `paintings`/changed with a `.why` beside each,
/// and the rest is removed.
fn once(job: &Job, options: &Options, log: &Path, paintings: &Path) -> Result<(), String> {
    let record = paintings.with_extension("run");
    let _ = fs::remove_dir_all(&record);
    // Each program the run drives writes its whole log here, beside the run's own output, so a
    // run stopped for passing its limit keeps them too; a run that passed has them removed.
    let logs = env::current_dir()
        .unwrap_or_default()
        .join(log.with_extension("logs"));
    let _ = fs::remove_dir_all(&logs);
    let (launcher, flags) = job
        .launcher
        .split_first()
        .ok_or("a test's [command] has no launcher")?;
    let mut command = Command::new(launcher);
    if let Some(latency) = options.latency {
        command.env("HOST_TEST_LATENCY_MS", latency.as_millis().to_string());
    }
    let ran = command
        .args(flags)
        // A later --time-limit is the one the launcher keeps.
        .args(["--time-limit", &format!("{}s", options.limit.as_secs())])
        .arg("--record")
        .arg(&record)
        .arg("--")
        .args(&job.command)
        .args(job.test.is_none().then_some(&options.name))
        .envs(job.environment.iter().map(|(key, value)| (key, value)))
        // be-tool-drive's `Driven` keeps them there (its `KEEP_LOGS`).
        .env("BE_DRIVE_KEEP_LOGS", &logs)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not run {launcher}: {error}"));
    let mut text = fs::read_to_string(record.join("log")).unwrap_or_default();
    let status = fs::read_to_string(record.join("status"))
        .ok()
        .and_then(|status| status.trim().parse::<i32>().ok());
    if let Ok(ran) = &ran {
        text.push_str(&String::from_utf8_lossy(&ran.stdout));
        text.push_str(&String::from_utf8_lossy(&ran.stderr));
    }
    let changed = compare(&record.join("made"), &job.accepted(), paintings);
    let _ = fs::remove_dir_all(&record);
    let _ = fs::remove_dir_all(paintings.join("used"));
    if changed.as_ref().is_ok_and(Vec::is_empty) {
        let _ = fs::remove_dir_all(paintings);
    }
    let failed = |reason: String| {
        let _ = fs::write(log, &text);
        Err(reason)
    };
    match status {
        Some(0) => match changed {
            Ok(changed) => match changed.into_iter().next() {
                None => {
                    let _ = fs::remove_dir_all(&logs);
                    Ok(())
                }
                Some((painting, why)) => failed(format!("the painting {painting} changed: {why}")),
            },
            Err(error) => failed(format!("the paintings could not be compared: {error}")),
        },
        Some(be_tool_run_test::TIMED_OUT) => failed(format!(
            "timed out: the run took more than {} seconds",
            options.limit.as_secs()
        )),
        Some(status) => {
            let ended = ended(status);
            failed(reason(&text, &ended))
        }
        None => failed(match ran {
            Err(error) => error,
            Ok(_) => reason(&text, "the launcher recorded no status"),
        }),
    }
}

/// Compares the paintings in `made` with those in `accepted` as verify does
/// (be-tool-paint-compare, `--update`): each that differs, by name with what changed, is in
/// `out`/changed with that in a `.why` beside it.
fn compare(made: &Path, accepted: &Path, out: &Path) -> Result<Vec<(String, String)>, String> {
    be_tool_paint_compare::run(&be_tool_paint_compare::Options {
        made: made.to_owned(),
        accepted: accepted.to_owned(),
        out: out.to_owned(),
        update: true,
        ..Default::default()
    })?;
    let changed = out.join("changed");
    let mut differ: Vec<(String, String)> = fs::read_dir(&changed)
        .map_err(|error| format!("could not read {}: {error}", changed.display()))?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let painting = name.strip_suffix(".why")?.to_owned();
            let why = fs::read_to_string(entry.path()).unwrap_or_default();
            Some((painting, why))
        })
        .collect();
    differ.sort();
    Ok(differ)
}

/// How a run that did not succeed ended, by the status the launcher recorded: 128 and the
/// signal for a test a signal stopped.
fn ended(status: i32) -> String {
    match status {
        129.. => format!("the harness was killed by signal {}", status - 128),
        _ => format!("the harness exited with status {status}"),
    }
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
