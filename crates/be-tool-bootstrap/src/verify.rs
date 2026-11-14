use std::process::{Command, ExitCode};

use crate::platform::target_platform;
use crate::quiet::Quiet;
use crate::{Result, buck, in_ci, lock, repository};

const VERIFY_USAGE: &str =
    "Usage: ./scripts/verify [--check] [--lint] [--tests] [--app-tests] [--thorough] [--verbose]";
const CI_USAGE: &str = "Usage: ./scripts/ci [--check] [--previews BASE OUT] [--bootstrap DIR]";

pub fn verify(arguments: Vec<String>) -> Result<ExitCode> {
    for argument in &arguments {
        if ![
            "--check",
            "--lint",
            "--tests",
            "--app-tests",
            "--thorough",
            "--verbose",
        ]
            .contains(&argument.as_str())
        {
            return Err(VERIFY_USAGE.to_owned());
        }
    }
    run(arguments)
}

pub fn ci(arguments: Vec<String>) -> Result<ExitCode> {
    let mut checked = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let values = match argument.as_str() {
            "--check" => 0,
            "--bootstrap" => 1,
            "--previews" => 2,
            _ => return Err(CI_USAGE.to_owned()),
        };
        checked.push(argument);
        for _ in 0..values {
            checked.push(arguments.next().ok_or_else(|| CI_USAGE.to_owned())?);
        }
    }
    run(checked)
}

fn run(arguments: Vec<String>) -> Result<ExitCode> {
    let has = |flag: &str| arguments.iter().any(|argument| argument == flag);
    let (mut lint, mut tests, mut app_tests) = (has("--lint"), has("--tests"), has("--app-tests"));
    let verbose = has("--verbose") || in_ci();
    let mut passed: Vec<String> = arguments
        .iter()
        .filter(|argument| *argument != "--verbose")
        .cloned()
        .collect();
    let thorough = has("--thorough");
    if !lint && !tests && !app_tests && !thorough {
        (lint, tests, app_tests) = (true, true, true);
        passed.extend(["--lint".to_owned(), "--app-tests".to_owned()]);
    }
    if verbose {
        // unsafe(env)
        unsafe { std::env::set_var("BE3_VERBOSE", "1") };
    }

    // Before buck2, which reads the downloads from the start.
    lock::ensure(has("--check"))?;
    lock::check_members()?;
    let buck2 = buck::prepare()?;
    buck::write_build_id()?;
    let target = repository().join("target");
    let manifest = target.join("verify-manifest.txt");
    let log = target.join("verify.log");
    let events = target.join("verify-events.json-lines");
    let _ = std::fs::remove_file(&events);
    passed.extend([
        "--event-log".to_owned(),
        events.to_string_lossy().into_owned(),
    ]);
    let host = target_platform().unwrap_or("linux_x86_64");
    let mut parts = Vec::new();
    for (flag, part) in [
        (lint, "lint"),
        (tests, "tests"),
        (app_tests, "app-tests"),
        (thorough, "thorough"),
        (has("--previews"), "previews"),
        (has("--bootstrap"), "bootstrap"),
    ] {
        if flag {
            parts.push(part);
        }
    }
    let (status, output) = buck::retrying(
        &buck2,
        &bxl_arguments(host, &parts, &events),
        false,
        true,
        (!verbose).then_some(log.as_path()),
    )?;
    std::fs::write(&manifest, &output)
        .map_err(|error| format!("could not write {}: {error}", manifest.display()))?;
    if status != 0 {
        passed.push("--build-failed".to_owned());
        if !verbose {
            let mut quiet = Quiet::new(false, 80);
            let text = std::fs::read(&log).unwrap_or_default();
            let mut stdout = std::io::stdout();
            for line in String::from_utf8_lossy(&text).lines() {
                quiet.line(line, &mut stdout);
            }
            quiet.finish(&mut stdout);
        }
    }

    let written = std::fs::read_to_string(&manifest).unwrap_or_default();
    let verify = written
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(kind, _)| *kind == "verify")
        .map(|(_, path)| path.trim_end_matches('\r').to_owned())
        .next_back();
    let Some(verify) = verify.filter(|path| std::path::Path::new(path).exists()) else {
        println!();
        println!("Verification failed: crates/be-tool-verify did not build.");
        return Ok(ExitCode::FAILURE);
    };
    let mut command = Command::new(verify);
    command
        .arg("verify")
        .arg(&manifest)
        .args(&passed)
        .current_dir(repository());
    buck::exec(command)
}

/// The buck2 command: buck/dev/verify.bxl with the parts asked for, which
/// prints the manifest crates/be-tool-verify reads, whole even when something
/// fails to build. --keep-going builds everything that can be, so that it
/// reads what did; the tools it runs here are built for host. The event log
/// is for crates/be-tool-verify too, which checks from it where Rust links
/// ran (be-tool-verify's links.rs).
fn bxl_arguments(host: &str, parts: &[&str], events: &std::path::Path) -> Vec<String> {
    let mut arguments: Vec<String> = [
        "bxl",
        "--keep-going",
        "--event-log",
        &events.to_string_lossy(),
        "//buck/dev/verify.bxl:main",
        "--",
        "--host",
        host,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for part in parts {
        arguments.extend(["--part".to_owned(), (*part).to_owned()]);
    }
    arguments
}
