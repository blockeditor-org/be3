use super::super::failures::{Unchecked, unchecked};

#[test]
fn a_failed_test_hides_only_its_own_packages_flaky_reports() {
    let event = |category: &str, package: &str, failed: bool| {
        format!(
            r#"{{"Event":{{"data":{{"SpanEnd":{{"data":{{"ActionExecution":{{"failed":{failed},"name":{{"category":"{category}","identifier":""}},"key":{{"owner":{{"TargetLabel":{{"label":{{"package":"root//{package}","name":"test"}}}}}}}}}}}}}}}}}}}}"#
        )
    };
    let tests_failed = [
        event("painting_test", "crates/a", true),
        event("painting_test", "crates/b", false),
        event("test_run", "crates/c", true),
    ]
    .join("\n");
    assert_eq!(
        unchecked(&tests_failed),
        Unchecked::Packages(["crates/a".to_owned(), "crates/c".to_owned()].into())
    );
    let compile_failed = [
        event("painting_test", "crates/a", true),
        event("rustc", "crates/d", true),
    ]
    .join("\n");
    assert_eq!(unchecked(&compile_failed), Unchecked::All);
    assert_eq!(unchecked(""), Unchecked::Packages(Default::default()));
}
