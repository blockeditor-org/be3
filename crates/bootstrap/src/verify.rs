use std::process::{Command, ExitCode};

use crate::platform::target_platform;
use crate::quiet::Quiet;
use crate::{Result, buck, in_ci, repository};

const VERIFY_USAGE: &str =
    "Usage: ./scripts/verify [--check] [--lint] [--tests] [--plugin-tests] [--verbose]";
const CI_USAGE: &str = "Usage: ./scripts/ci [--check] [--previews BASE OUT] [--bootstrap DIR]";

pub fn verify(arguments: Vec<String>) -> Result<ExitCode> {
    for argument in &arguments {
        if ![
            "--check",
            "--lint",
            "--tests",
            "--plugin-tests",
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
    let (mut lint, mut tests, mut plugin_tests) =
        (has("--lint"), has("--tests"), has("--plugin-tests"));
    let verbose = has("--verbose") || in_ci();
    let mut passed: Vec<String> = arguments
        .iter()
        .filter(|argument| *argument != "--verbose")
        .cloned()
        .collect();
    if !lint && !tests && !plugin_tests {
        (lint, tests, plugin_tests) = (true, true, true);
        passed.extend(["--lint".to_owned(), "--plugin-tests".to_owned()]);
    }
    if verbose {
        // unsafe(env)
        unsafe { std::env::set_var("BE3_VERBOSE", "1") };
    }

    let buck2 = buck::prepare()?;
    buck::write_build_id()?;
    let target = repository().join("target");
    let manifest = target.join("verify-manifest.txt");
    let log = target.join("verify.log");
    let host = target_platform().unwrap_or("linux_x86_64");
    let bxl: Vec<String> = [
        "bxl",
        "--keep-going",
        "//buck/dev/verify.bxl:main",
        "--",
        "--host",
        host,
        "--lint",
        bool(lint),
        "--tests",
        bool(tests),
        "--plugin_tests",
        bool(plugin_tests),
        "--previews",
        bool(has("--previews")),
        "--bootstrap",
        bool(has("--bootstrap")),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let (status, output) = buck::retrying(
        &buck2,
        &bxl,
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

    let verify = String::from_utf8_lossy(&output)
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(kind, _)| *kind == "verify")
        .map(|(_, path)| path.trim_end_matches('\r').to_owned())
        .next_back();
    let Some(verify) = verify.filter(|path| std::path::Path::new(path).exists()) else {
        println!();
        println!("Verification failed: crates/verify did not build.");
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

fn bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
