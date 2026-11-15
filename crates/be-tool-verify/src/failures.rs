//! Which outputs of a test this build can stand behind. A test's outputs are at
//! fixed paths, so one that did not run this time leaves an earlier build's
//! there: its paintings (`compared`) and the report of its second run
//! (_again in buck/dev/test.bzl). When everything built, every output is this
//! build's. When something failed, only a test this build compared the
//! paintings of is known to have run: one whose analysis failed shows no
//! failed action at all, and one that was up to date shows none either, its
//! outputs already offered by the build that made them. Of those, a failed
//! test hides only its own package's reports; anything else that failed, a
//! compile say, may have kept a test from running, so it hides every report.

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

/// The tests, as "crates/foo:name", whose paintings this build compared: a
/// painting_compare action (run_test in buck/dev/test.bzl) that ended without
/// failing.
pub fn compared(events: &str) -> BTreeSet<String> {
    let mut tests = BTreeSet::new();
    for line in events.lines() {
        if !line.contains("\"painting_compare\"") || !line.contains("\"failed\":false") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let action = &event["Event"]["data"]["SpanEnd"]["data"]["ActionExecution"];
        if action["failed"] != false || action["name"]["category"] != "painting_compare" {
            continue;
        }
        let label = &action["key"]["owner"]["TargetLabel"]["label"];
        if let (Some(package), Some(name)) = (label["package"].as_str(), label["name"].as_str()) {
            tests.insert(format!(
                "{}:{name}",
                package.strip_prefix("root//").unwrap_or(package)
            ));
        }
    }
    tests
}

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
