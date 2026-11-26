//! git, run in the current directory for what it prints.

use std::process::{Command, Stdio};

/// What `git ARGUMENTS` prints, or `None` when it cannot run, fails or prints something that
/// is not UTF-8. Its errors are dropped.
pub fn git(arguments: &[&str]) -> Option<String> {
    git_bytes(arguments).and_then(|bytes| String::from_utf8(bytes).ok())
}

/// The bytes `git ARGUMENTS` prints, or `None` when it cannot run or fails.
pub fn git_bytes(arguments: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(arguments)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}
