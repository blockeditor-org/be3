use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use crate::pins::Pins;
use crate::{Result, be, buck2};

const PACKAGES: &[&str] = &[
    "curl",
    "git",
    "python3",
    "libasound2t64",
    "libgtk-3-0t64",
    "libwebkit2gtk-4.1-0",
    "libvulkan1",
    "mesa-vulkan-drivers",
    "libx11-6",
    "libxcursor1",
    "libxrandr2",
    "libxi6",
    "libxkbcommon0",
    "libxkbcommon-x11-0",
    "libwayland-client0",
    "fonts-dejavu-core",
    "fonts-dejavu-mono",
    "libgbm1",
    "libinput10",
    "libseat1",
    "libudev1",
];

struct Steps {
    started: Instant,
    step: Option<(String, Instant)>,
}

impl Steps {
    fn step(&mut self, name: &str) {
        self.end();
        println!("{name}...");
        self.step = Some((name.to_owned(), Instant::now()));
    }

    fn end(&mut self) {
        if let Some((name, started)) = self.step.take() {
            println!("  {name} took {}", format_duration(started.elapsed()));
        }
    }
}

impl Drop for Steps {
    fn drop(&mut self) {
        self.end();
        println!("Setup took {}", format_duration(self.started.elapsed()));
    }
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_millis();
    if total >= 60_000 {
        format!("{}m {:02}s", total / 60_000, total % 60_000 / 1000)
    } else {
        format!("{}.{}s", total / 1000, total % 1000 / 100)
    }
}

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    if !arguments.is_empty() {
        return Err("Usage: ./scripts/setup".to_owned());
    }
    let mut steps = Steps {
        started: Instant::now(),
        step: None,
    };
    let has = |program: &str| {
        Command::new(program)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    };
    if std::env::consts::OS != "linux" || !has("apt-get") {
        return Err(
            "This script only knows how to set up Ubuntu. Install the equivalents of\n\
                    the packages it lists by hand, then run ./scripts/buck, which installs buck2."
                .to_owned(),
        );
    }
    let root = Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "0");
    if !root && !has("sudo") {
        return Err("sudo was not found on PATH. Run this script as root instead.".to_owned());
    }
    let privileged = |arguments: &[&str]| {
        let mut command = if root {
            Command::new(arguments[0])
        } else {
            let mut sudo = Command::new("sudo");
            sudo.arg(arguments[0]);
            sudo
        };
        command
            .args(&arguments[1..])
            .env("DEBIAN_FRONTEND", "noninteractive");
        command.status().is_ok_and(|status| status.success())
    };

    let missing: Vec<&str> = PACKAGES
        .iter()
        .copied()
        .filter(|package| {
            !Command::new("dpkg-query")
                .args(["--show", "--showformat=${db:Status-Status}\n", package])
                .stderr(Stdio::null())
                .output()
                .is_ok_and(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .any(|line| line == "installed")
                })
        })
        .collect();
    if missing.is_empty() {
        println!(
            "All {} system packages are already installed.",
            PACKAGES.len()
        );
    } else {
        steps.step(&format!("Installing {} system packages", missing.len()));
        if !privileged(&["apt-get", "update"]) {
            return Err("apt-get update failed.".to_owned());
        }
        let mut install = vec!["apt-get", "install", "-y"];
        install.extend(&missing);
        if !privileged(&install) {
            return Err("apt-get install failed.".to_owned());
        }
    }

    let pins = Pins::read()?;
    steps.step("Installing buck2");
    let buck2 = buck2::ensure(&pins)?;
    println!("buck2 is installed at {}", buck2.display());

    steps.step("Installing be");
    if pins.get("be_release").is_ok() {
        let be = be::ensure(&pins)?;
        println!("be is installed at {}", be.display());
    } else {
        println!("No be release is pinned yet, so ./scripts/be is not installed.");
    }

    steps.end();
    println!();
    println!("Setup finished. The first time ./scripts/buck runs, it asks which build server");
    println!("to build on and for its key - guides/build_server.md says more. Then check the");
    println!("workspace compiles with");
    println!("./scripts/buck run //:check, and run ./scripts/verify before committing.");
    Ok(ExitCode::SUCCESS)
}
