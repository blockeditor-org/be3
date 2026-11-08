//! ./scripts/buckify: the crates' facts (each crate's PACKAGE, the members
//! index and third-party/rust/crates.bzl) and Cargo.lock brought up to date
//! with the Cargo.toml files, as ./scripts/verify does, so that a manifest
//! change builds before then. The lock step first (lock.rs), with the network,
//! then one buck2 command, buck/cargo/buckify.bxl, which generates the facts
//! on a worker from cargo's own plans and builds crates/be-tool-verify for this
//! machine, and then that be-tool-verify's `buckify`, which writes them into
//! the checkout. With --check nothing is written, and it fails if anything is
//! out of date.
//!
//! When the bxl fails, such as when the checked-in facts cannot build
//! crates/be-tool-buck because its own dependencies changed, it runs once more
//! with be-tool-buck built by cargo inside the action (buck/cargo/buckify.bzl).

use std::process::{Command, ExitCode};

use crate::platform::target_platform;
use crate::{Result, buck, lock, repository};

const USAGE: &str = "Usage: ./scripts/buckify [--check]";

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let check = match arguments.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err(USAGE.to_owned()),
    };
    lock::ensure(check)?;
    let buck2 = buck::prepare()?;
    buck::write_build_id()?;
    let host = target_platform().ok_or("No target platform is defined for this machine.")?;
    let bxl = |cargo_built: bool| -> Result<(i32, Vec<u8>)> {
        let mut arguments: Vec<String> = [
            "bxl",
            "root//buck/cargo/buckify.bxl:main",
            "--",
            "--platform",
            &format!("root//buck/platforms:{host}"),
        ]
        .map(str::to_owned)
        .to_vec();
        if cargo_built {
            arguments.extend(["--cargo_built".to_owned(), "true".to_owned()]);
        }
        buck::retrying(&buck2, &arguments, false, true, None)
    };
    let (mut status, mut output) = bxl(false)?;
    if status != 0 {
        eprintln!(
            "buckify could not plan with crates/be-tool-buck built from the checked-in facts; \
             planning again with it built by cargo."
        );
        (status, output) = bxl(true)?;
    }
    if status != 0 {
        return Ok(buck::exit_code(status));
    }
    let output = String::from_utf8_lossy(&output);
    let line = |kind: &str| {
        output
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(each, _)| *each == kind)
            .map(|(_, path)| path.trim_end_matches('\r').to_owned())
            .ok_or_else(|| format!("buck/cargo/buckify.bxl did not print its {kind} line."))
    };
    let mut command = Command::new(line("verify")?);
    command
        .arg("buckify")
        .arg(line("generated")?)
        .args(check.then_some("--check"))
        .current_dir(repository());
    buck::exec(command)
}
