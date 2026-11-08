use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use crate::quiet::Quiet;
use crate::{Result, buck, repository};

const USAGE: &str = "Usage: ./scripts/test NAME [--update]\n       ./scripts/test //crates/<crate>:test [NAME] [--update]";

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let mut name = String::new();
    let mut update = false;
    let mut targets = Vec::new();
    for argument in arguments {
        if argument == "--update" {
            update = true;
        } else if argument.starts_with("//") {
            targets.push(argument);
        } else if argument.starts_with('-') || !name.is_empty() {
            return Err(USAGE.to_owned());
        } else {
            name = argument;
        }
    }
    if name.is_empty() && targets.is_empty() {
        return Err(USAGE.to_owned());
    }

    if targets.is_empty() {
        if !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == ':'
        }) {
            return Err(format!(
                "A test name is letters, digits, underscores and ::, not '{name}'."
            ));
        }
        let crates = crates_with(&name);
        if crates.is_empty() {
            return Err(format!(
                "No test in crates/ has a name containing '{name}'.\n\
                 A test's name is its module path and its function, such as media::tests::plays::plays for\n\
                 crates/be-host/src/media/tests/plays.rs; NAME may be any part of one."
            ));
        }
        for krate in crates {
            let rules =
                std::fs::read_to_string(repository().join(&krate).join("BUCK")).unwrap_or_default();
            let has_test = has_test_target(&rules);
            if !has_test {
                eprintln!(
                    "Skipping {krate}: it holds a test whose name contains '{name}' but has no :test target in its BUCK."
                );
                continue;
            }
            targets.push(format!("//{krate}:test"));
        }
        if targets.is_empty() {
            return Err(String::new());
        }
    }
    eprintln!("Running {}", targets.join(" "));
    // unsafe(env)
    unsafe { std::env::set_var("BE3_VERBOSE", "1") };
    let buck2 = buck::prepare()?;
    buck::write_build_id()?;
    let failed = if update {
        update_paintings(&buck2, &targets, name)?
    } else {
        run(&buck2, &targets, &name)?
    };
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// A plain run is `buck2 test`, which runs each test where its rule says, from
/// the cache when a worker passed it before, and keeps every test's output in
/// its event log, where the console shows only a failure's: each is printed
/// from there. A run_apart crate's :test is its parts, each `:test-<file>`
/// whose test contains NAME. libtest takes every filter as one more to match,
/// so NAME goes to each test, and a part names its own test with --exact
/// (cargo_test in buck/cargo/defs.bzl), which NAME, a part of that name, does
/// not widen.
fn run(buck2: &Path, targets: &[String], name: &str) -> Result<bool> {
    let mut chosen = Vec::new();
    for target in targets {
        match parts(target, name) {
            Some(parts) => chosen.extend(parts),
            None => chosen.push(target.clone()),
        }
    }
    let target = repository().join("target");
    let events = target.join("test-events.json-lines");
    let log = target.join("test.log");
    let _ = std::fs::remove_file(&events);
    let mut arguments: Vec<String> = vec![
        "test".into(),
        "--unstable-allow-compatible-tests-on-re".into(),
        "--event-log".into(),
        events.to_string_lossy().into_owned(),
    ];
    arguments.extend(chosen.iter().cloned());
    // --test-arg takes everything after it.
    arguments.extend(["--".into(), "--test-arg".into(), "--nocapture".into()]);
    if !name.is_empty() {
        arguments.push(name.to_owned());
    }
    let (status, _) = buck::retrying(buck2, &arguments, false, true, Some(&log))?;
    let results = results(&events)?;
    let mut failed = status != 0;
    let mut reported = BTreeSet::new();
    for result in &results {
        let passed = result.status == PASS || result.status == SKIP || result.status == OMITTED;
        failed |= !passed;
        println!("\n{} {}:", result.name, status_name(result.status));
        let details = result.details.trim_end();
        let details = details
            .strip_suffix("---- STDERR ----")
            .map_or(details, str::trim_end);
        println!("{details}");
        reported.insert(result.name.trim_start_matches("root").to_owned());
    }
    let unreported = chosen
        .iter()
        .any(|target| !reported.contains(target.trim_start_matches("root")));
    if status != 0 && (results.is_empty() || unreported) {
        // What buck2 said, such as why a test did not build.
        let mut quiet = Quiet::new(false, 80);
        let text = std::fs::read(&log).unwrap_or_default();
        let mut stdout = std::io::stdout();
        println!();
        for line in String::from_utf8_lossy(&text).lines() {
            quiet.line(line, &mut stdout);
        }
        quiet.finish(&mut stdout);
    }
    Ok(failed)
}

/// buck2's TestStatus, as its event log writes it.
const PASS: i64 = 1;
const SKIP: i64 = 3;
const OMITTED: i64 = 4;

fn status_name(status: i64) -> &'static str {
    match status {
        PASS => "passed",
        2 => "failed",
        SKIP => "skipped",
        OMITTED => "omitted",
        5 => "failed fatally",
        6 => "timed out",
        _ => "did not finish",
    }
}

struct TestResult {
    name: String,
    status: i64,
    details: String,
}

/// Every TestResult in a buck2 event log written as JSON lines.
fn results(events: &Path) -> Result<Vec<TestResult>> {
    let text = std::fs::read_to_string(events).unwrap_or_default();
    let mut results = Vec::new();
    for line in text.lines() {
        if !line.contains("\"TestResult\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let result = &event["Event"]["data"]["Instant"]["data"]["TestResult"];
        let Some(name) = result["name"].as_str() else {
            continue;
        };
        results.push(TestResult {
            name: name.to_owned(),
            status: result["status"].as_i64().unwrap_or_default(),
            details: result["details"].as_str().unwrap_or_default().to_owned(),
        });
    }
    Ok(results)
}

/// A run_apart crate's :test as its parts whose test's name contains NAME,
/// every one without a NAME, as buck/cargo/defs.bzl makes them: one
/// `:test-<file>` for each file under src/tests/, whose test is
/// tests::<file>::<file>. None for any other target, or when no part matches.
fn parts(target: &str, name: &str) -> Option<Vec<String>> {
    let krate = target.strip_prefix("//")?.strip_suffix(":test")?;
    let directory = repository().join(krate);
    let rules = std::fs::read_to_string(directory.join("BUCK")).ok()?;
    if !rules.lines().any(|line| line.trim() == "run_apart = True,") {
        return None;
    }
    let mut parts: Vec<String> = std::fs::read_dir(directory.join("src/tests"))
        .ok()?
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|file| file.strip_suffix(".rs").map(str::to_owned))
        .filter(|part| name.is_empty() || format!("tests::{part}::{part}").contains(name))
        .map(|part| format!("//{krate}:test-{part}"))
        .collect();
    parts.sort();
    (!parts.is_empty()).then_some(parts)
}

/// --update runs each test as an action through buck/dev/test.bxl, which keeps
/// the paintings it accepted for this to copy into the crate's snapshots/. An
/// action's failure is cached like a pass, so a run asked for again after one
/// failed adds --nonce, which goes into every test's environment.
fn update_paintings(buck2: &Path, targets: &[String], name: String) -> Result<bool> {
    let mut arguments: Vec<String> = ["bxl", "--keep-going", "//buck/dev/test.bxl:main", "--"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for target in targets {
        arguments.extend(["--target".to_owned(), target.clone()]);
    }
    arguments.extend([
        "--name".to_owned(),
        name,
        "--update".to_owned(),
        "true".to_owned(),
    ]);
    let failures = repository().join("target/test-failed");
    let asked = arguments.join(" ");
    if std::fs::read_to_string(&failures).is_ok_and(|last| last == asked) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or_default();
        arguments.extend(["--nonce".to_owned(), nonce.to_string()]);
    }
    let (status, output) = buck::retrying(buck2, &arguments, false, true, None)?;
    let mut failed = status != 0;
    for line in String::from_utf8_lossy(&output).lines() {
        let Some((target, directory)) = line.trim_end_matches('\r').split_once('\t') else {
            continue;
        };
        failed |= !report(target, Path::new(directory));
    }
    let _ = if failed {
        std::fs::write(&failures, asked)
    } else {
        std::fs::remove_file(&failures)
    };
    Ok(failed)
}

/// Whether a crate's BUCK makes a :test: cargo_test, an app's or app_tests'
/// wasm tests, or a macro that makes one unless told `test = False`
/// (buck/cargo/defs.bzl, buck/wasm/defs.bzl).
pub fn has_test_target(rules: &str) -> bool {
    rules.lines().any(|line| {
        ["cargo_test(", "app(", "app_tests(", "game_module("]
            .iter()
            .any(|rule| line.starts_with(rule))
            || (["cargo_crate(", "type_module("]
                .iter()
                .any(|rule| line.starts_with(rule))
                && !line.contains("test = False"))
    })
}

fn report(target: &str, directory: &Path) -> bool {
    let status = std::fs::read_to_string(directory.join("status")).unwrap_or_default();
    let passed = status.trim() == "0";
    println!("\n{target} {}:", if passed { "passed" } else { "failed" });
    let log = std::fs::read(directory.join("log")).unwrap_or_default();
    println!("{}", String::from_utf8_lossy(&log).trim_end());
    let mut accepted = passed;
    let mut paintings: Vec<_> = std::fs::read_dir(directory.join("changed"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "paint")
        })
        .collect();
    paintings.sort();
    let package = target
        .split_once("//")
        .map_or(target, |(_, label)| label)
        .split(':')
        .next()
        .unwrap_or_default();
    let snapshots = repository().join(package).join("snapshots");
    for painting in paintings {
        let Some(name) = painting.file_name() else {
            continue;
        };
        let mut why = painting.clone().into_os_string();
        why.push(".why");
        let why = std::fs::read_to_string(why).unwrap_or_default();
        let to = snapshots.join(name);
        let name = format!("{package}/snapshots/{}", name.to_string_lossy());
        match std::fs::create_dir_all(&snapshots).and_then(|()| std::fs::copy(&painting, &to)) {
            Ok(_) => println!("Accepted {name}: {}", why.trim_end()),
            Err(error) => {
                println!("Could not write {name}: {error}");
                accepted = false;
            }
        }
    }
    accepted
}

fn crates_with(name: &str) -> BTreeSet<String> {
    let mut files = Vec::new();
    collect_rust_files(&repository().join("crates"), &mut files);
    let mut crates = BTreeSet::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        if !text.contains("#[test") && !text.contains("#[tokio::test") {
            continue;
        }
        let Ok(relative) = file.strip_prefix(repository()) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        for function in function_names(&text) {
            if let Some(krate) = crate_of_function(&relative, function, name) {
                crates.insert(krate);
            }
        }
    }
    crates
}

fn collect_rust_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn function_names(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("fn ").filter_map(|(at, _)| {
        let rest = &text[at + 3..];
        let length = rest
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(rest.len());
        (length > 0).then(|| &rest[..length])
    })
}

pub fn crate_of_function(file: &str, function: &str, name: &str) -> Option<String> {
    let (krate, mut relative) = if let Some(at) = file.find("/src/") {
        (&file[..at], file[at + 5..].to_owned())
    } else {
        let at = ["/tests/", "/examples/", "/benches/"]
            .iter()
            .filter_map(|marker| file.find(marker).map(|at| (at, marker.len())))
            .min()?;
        let after = &file[at.0 + at.1..];
        let after = match after.find('/') {
            Some(slash) => &after[slash + 1..],
            None => "",
        };
        (&file[..at.0], after.to_owned())
    };
    if let Some(stripped) = relative.strip_suffix(".rs") {
        relative = stripped.to_owned();
    }
    if let Some(rest) = relative.strip_prefix("bin/") {
        relative = match rest.find('/') {
            Some(slash) => rest[slash + 1..].to_owned(),
            None => String::new(),
        };
    }
    if relative == "mod" {
        relative.clear();
    } else if let Some(stripped) = relative.strip_suffix("/mod") {
        relative = stripped.to_owned();
    }
    if relative == "lib" || relative == "main" {
        relative.clear();
    }
    let relative = relative.replace('/', "::");
    let full = if relative.is_empty() {
        function.to_owned()
    } else {
        format!("{relative}::{function}")
    };
    full.contains(name).then(|| krate.to_owned())
}
