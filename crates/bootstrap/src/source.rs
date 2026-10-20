use std::process::{Command, ExitCode};

use crate::platform::target_platform;
use crate::{Result, buck};

pub fn run_from_source(command: &str, arguments: &[String]) -> Option<ExitCode> {
    if std::env::var("BE3_BOOTSTRAP").ok()? != "source" {
        return None;
    }
    Some(match build_and_run(command, arguments) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    })
}

fn build_and_run(command: &str, arguments: &[String]) -> Result<ExitCode> {
    let buck2 = buck::prepare()?;
    buck::write_build_id()?;
    let host = target_platform().ok_or("No target platform is defined for this machine.")?;
    let build = [
        "build".to_owned(),
        "--target-platforms".to_owned(),
        format!("root//buck/platforms:{host}"),
        "//crates/bootstrap:bin".to_owned(),
        "--show-full-simple-output".to_owned(),
    ];
    eprintln!(
        "BE3_BOOTSTRAP=source: building crates/bootstrap to run it in place of the pinned release."
    );
    let (status, output) = buck::retrying(&buck2, &build, false, true, None)?;
    if status != 0 {
        return Ok(buck::exit_code(status));
    }
    let built = String::from_utf8_lossy(&output).trim().to_owned();
    let mut run = Command::new(built);
    run.arg(command)
        .args(arguments)
        .env("BE3_BOOTSTRAP", "built");
    buck::exec(run)
}
