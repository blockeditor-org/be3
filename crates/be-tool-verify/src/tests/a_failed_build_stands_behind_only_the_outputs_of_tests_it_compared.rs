use std::path::Path;

use super::super::failures::compared;
use super::super::this_builds;

/// A test's outputs are at fixed paths, so one whose analysis failed leaves an earlier build's
/// there without a failed action to show it: when the build failed, only the outputs of a test
/// whose paintings this build compared are its own.
#[test]
fn a_failed_build_stands_behind_only_the_outputs_of_tests_it_compared() {
    let event = |category: &str, package: &str, name: &str, failed: bool| {
        format!(
            r#"{{"Event":{{"data":{{"SpanEnd":{{"data":{{"ActionExecution":{{"failed":{failed},"name":{{"category":"{category}","identifier":""}},"key":{{"owner":{{"TargetLabel":{{"label":{{"package":"root//{package}","name":"{name}"}}}}}}}}}}}}}}}}}}}}"#
        )
    };
    let events = [
        event("painting_compare", "crates/a", "test", false),
        event("painting_compare", "crates/b", "test", true),
        event("painting_test", "crates/c", "test", false),
        event("painting_compare", "crates/host", "test-opens_a_file", false),
    ]
    .join("\n");
    let ran = compared(&events);
    assert_eq!(
        ran,
        ["crates/a:test".to_owned(), "crates/host:test-opens_a_file".to_owned()].into()
    );

    let path = |package: &str, name: &str, output: &str| {
        Path::new("/buck-out/v2/art/root/cfg")
            .join(package)
            .join(format!("__{name}__"))
            .join(output)
    };
    let this = |package: &str, name: &str, output: &str| {
        this_builds(Some(&ran), Path::new(package), &path(package, name, output))
    };
    assert!(this("crates/a", "test", "paintings"));
    assert!(this("crates/a", "test", "paintings-again"), "its second run's report too");
    assert!(this("crates/host", "test-opens_a_file", "paintings"));
    assert!(!this("crates/b", "test", "paintings"), "its compare failed");
    assert!(!this("crates/c", "test", "paintings"), "it ran but did not compare");
    assert!(!this("crates/d", "test", "paintings"), "it did not run at all");
    assert!(!this("crates/host", "test-another", "paintings"));
    assert!(
        this_builds(None, Path::new("crates/d"), &path("crates/d", "test", "paintings")),
        "when everything built, every output is this build's"
    );
}
