use std::process::{Command, ExitCode};

use crate::{Result, buck, repository};

const USAGE: &str = "Usage: ./scripts/switch <branch> [script args...]

Fetches from origin and checks out origin/<branch> in detached HEAD state.
Any arguments after <branch> name a script in scripts/ and its arguments, run once
the checkout succeeds, e.g. ./scripts/switch somebranch buck run //crates/beui:example-dock";

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let mut arguments = arguments.into_iter();
    let branch = match arguments.next() {
        Some(help) if help == "-h" || help == "--help" => {
            println!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        }
        Some(branch) if !branch.is_empty() => branch,
        _ => return Err(USAGE.to_owned()),
    };
    let git = |arguments: &[&str]| {
        Command::new("git")
            .args(arguments)
            .current_dir(repository())
            .status()
            .map_err(|error| format!("could not run git: {error}"))
            .map(buck::status_code)
    };
    let fetched = git(&["fetch"])?;
    if fetched != 0 {
        return Ok(buck::exit_code(fetched));
    }
    let checked_out = git(&[
        "-c",
        "advice.detachedHead=false",
        "checkout",
        &format!("origin/{branch}"),
    ])?;
    if checked_out != 0 {
        return Ok(buck::exit_code(checked_out));
    }
    let Some(script) = arguments.next() else {
        return Ok(ExitCode::SUCCESS);
    };
    let mut command = Command::new("bash");
    command
        .arg(repository().join("scripts").join(script))
        .args(arguments);
    buck::exec(command)
}
