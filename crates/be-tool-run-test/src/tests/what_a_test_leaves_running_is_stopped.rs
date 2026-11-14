use std::path::Path;
use std::process::Command;

use super::scratch;

/// Whether a process is still there to signal: one the launcher reaped is gone, while one left running, or one
/// orphaned to another process, is not.
fn running(pid: &str) -> bool {
    Path::new("/proc").join(pid).exists()
}

#[test]
fn what_a_test_leaves_running_is_stopped() {
    // The launcher's main adopts and stops orphans, so this runs the built launcher (RUN_TEST, buck's :bin).
    let launcher = std::env::var("RUN_TEST").expect("RUN_TEST names the built run-test");
    let directory = scratch("leftovers");
    let (grouped, detached) = (directory.join("grouped"), directory.join("detached"));
    let script = format!(
        "sleep 1000 & echo $! > '{}'; setsid sleep 1000 & echo $! > '{}'; exit 0",
        grouped.display(),
        detached.display()
    );
    let status = Command::new(launcher)
        .args(["--", "sh", "-c", &script])
        .status()
        .unwrap();
    assert!(status.success(), "{status}");
    for (what, file) in [("in the test's group", &grouped), ("in a session of its own", &detached)] {
        let pid = std::fs::read_to_string(file).unwrap();
        let pid = pid.trim();
        assert!(!running(pid), "the sleeper {what} ({pid}) is still running");
    }
}
