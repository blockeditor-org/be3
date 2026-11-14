use std::ffi::OsString;
use std::path::PathBuf;

use crate::{Options, parse, run};

mod durations_are_seconds_minutes_or_hours;
mod flags_come_before_the_command;
mod library_path_reaches_the_test;
mod one_test_fails_a_harness_that_ran_none;
mod paintings_names_the_made_directory;
mod the_cpu_a_test_used_is_reported;
mod passed_is_written_only_when_the_test_passes;
mod record_keeps_the_output_and_status_and_exits_zero;
mod the_test_gets_a_short_tmpdir_removed_after;
mod the_time_limit_stops_the_test;

/// An empty directory of the test's own.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("run-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn shell(script: &str) -> Vec<OsString> {
    ["sh", "-c", script]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn options(command: Vec<OsString>) -> Options {
    Options {
        command,
        ..Options::default()
    }
}

fn run_ok(options: &Options) -> i32 {
    run(options).unwrap()
}

fn arguments(text: &[&str]) -> Result<Options, String> {
    parse(text.iter().map(OsString::from))
}
