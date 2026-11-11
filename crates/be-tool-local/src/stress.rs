use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, thread};

const USAGE: &str = "usage: local-tools stress NAME [--repeat N] [--parallel K]";
const HARNESS: &str = "//crates/be-host-tests:test-harness";
const BUNDLE: &str = "//crates/be-host-tests:host-with-all";
const HOST: &str = "//crates/be-host:bin";
const AMD64: &str = "//buck/sysroot:amd64";
const AMD64_TEST: &str = "//buck/sysroot:amd64-test";
const LAVAPIPE: &str = "//buck/sysroot:lavapipe_icd.json";
const REPEAT: usize = 20;
const PARALLEL: usize = 4;

/// Where a failing run's output goes, one file each.
const OUTPUT: &str = "target/stress";

/// Runs the host tests whose names contain NAME again and again, several at once, and says how
/// many failed and why: a flake that shows only under load shows here (guides/testing_a_gui.md,
/// "Finding a flake"). `./scripts/test` caches a passing run, so running it again proves
/// nothing.
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

struct Options {
    name: String,
    repeat: usize,
    parallel: usize,
}

fn options(arguments: Vec<String>) -> Result<Options, String> {
    let mut name = None;
    let mut repeat = REPEAT;
    let mut parallel = PARALLEL;
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
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}\n{USAGE}")),
            _ if name.is_some() => return Err(USAGE.to_owned()),
            _ => name = Some(argument),
        }
    }
    Ok(Options {
        name: name.ok_or_else(|| USAGE.to_owned())?,
        repeat,
        parallel,
    })
}

fn run(arguments: Vec<String>) -> Result<usize, String> {
    let options = options(arguments)?;
    let built = build()?;
    let output = PathBuf::from(OUTPUT);
    let _ = fs::remove_dir_all(&output);
    fs::create_dir_all(&output)
        .map_err(|error| format!("could not make {}: {error}", output.display()))?;
    let next = AtomicUsize::new(0);
    let reasons: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());
    let failed = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..options.parallel.min(options.repeat) {
            scope.spawn(|| {
                loop {
                    let run = next.fetch_add(1, Ordering::SeqCst);
                    if run >= options.repeat {
                        return;
                    }
                    let log = output.join(format!("{run}.log"));
                    match once(&built, &options.name, &log) {
                        Ok(()) => {
                            let _ = fs::remove_file(&log);
                        }
                        Err(reason) => {
                            failed.fetch_add(1, Ordering::SeqCst);
                            *reasons
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .entry(reason)
                                .or_default() += 1;
                        }
                    }
                }
            });
        }
    });
    let failed = failed.load(Ordering::SeqCst);
    println!(
        "{}: {} of {} runs failed, {} at a time",
        options.name, failed, options.repeat, options.parallel
    );
    for (reason, count) in reasons
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        println!("  {count} x {reason}");
    }
    if failed > 0 {
        println!("The failing runs' output is in {OUTPUT}/.");
    }
    Ok(failed)
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
    Ok(Built {
        harness: path(HARNESS)?,
        environment: vec![
            ("BE_BUNDLE", path(BUNDLE)?.display().to_string()),
            ("BE_HOST", path(HOST)?.display().to_string()),
            (
                "HOST_LIBRARIES",
                format!(
                    "{}:{}",
                    libraries(&amd64_test).display(),
                    libraries(&amd64).display()
                ),
            ),
            ("VK_ICD_FILENAMES", path(LAVAPIPE)?.display().to_string()),
            (
                "XKB_CONFIG_ROOT",
                amd64_test.join("usr/share/X11/xkb").display().to_string(),
            ),
            (
                "CARGO_MANIFEST_DIR",
                env::current_dir()
                    .map_err(|error| format!("no working directory: {error}"))?
                    .join("crates/be-host-tests")
                    .display()
                    .to_string(),
            ),
        ],
    })
}

/// One run of the tests named, which fails with the first line of what went wrong.
fn once(built: &Built, name: &str, log: &Path) -> Result<(), String> {
    let output = Command::new(&built.harness)
        .arg(name)
        .envs(built.environment.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not run the harness: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::write(log, &text);
    Err(reason(&text))
}

/// The panic's own message, which tells failures apart.
fn reason(text: &str) -> String {
    let mut lines = text.lines();
    lines
        .find(|line| line.contains(" panicked at "))
        .and_then(|_| lines.next())
        .map_or_else(
            || "failed without a panic".to_owned(),
            |message| message.trim().to_owned(),
        )
}
