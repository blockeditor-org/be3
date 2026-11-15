//! Which reports of a test's second run (_again in buck/dev/test.bzl) this
//! build can stand behind. A test whose run failed leaves an earlier build's
//! report at its path, so a failed test hides only its own package's; anything
//! else that failed, a compile say, may have kept a test from running at all,
//! leaving its report as it was without a failure of its own to show it, so it
//! hides every report.

use std::collections::BTreeSet;

/// The categories of the actions that run a test or compare what it painted
/// (run_test and _again in buck/dev/test.bzl).
const TEST_ACTIONS: &[&str] = &[
    "test",
    "test_run",
    "painting_test",
    "painting_compare",
    "painting_test_again",
    "painting_flaky",
];

/// Whose reports to leave unread.
#[derive(Debug, PartialEq)]
pub enum Unchecked {
    /// Every one.
    All,
    /// Those of these packages, as `crates/foo`.
    Packages(BTreeSet<String>),
}

/// What failed in the event log, written as JSON lines, says about the
/// reports.
pub fn unchecked(events: &str) -> Unchecked {
    let mut packages = BTreeSet::new();
    for line in events.lines() {
        if !line.contains("\"ActionExecution\"") || !line.contains("\"failed\":true") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return Unchecked::All;
        };
        let action = &event["Event"]["data"]["SpanEnd"]["data"]["ActionExecution"];
        if action["failed"] != true {
            continue;
        }
        let category = action["name"]["category"].as_str().unwrap_or_default();
        let package = action["key"]["owner"]["TargetLabel"]["label"]["package"].as_str();
        match package {
            Some(package) if TEST_ACTIONS.contains(&category) => {
                packages.insert(package.strip_prefix("root//").unwrap_or(package).to_owned());
            }
            _ => return Unchecked::All,
        }
    }
    Unchecked::Packages(packages)
}
