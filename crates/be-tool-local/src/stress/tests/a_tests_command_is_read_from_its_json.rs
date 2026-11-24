use super::*;

#[test]
fn a_tests_command_is_read_from_its_json() {
    let text = r#"{
        "command": ["buck-out/harness", "--exact", "tests::a::a"],
        "env": {"BE_HOST": ["buck-out/be-host"], "CARGO_MANIFEST_DIR": ["crates/be-host-tests"]},
        "launcher": ["buck-out/run_test", "--time-limit", "20m", "--one-test"]
    }"#;
    let job = Job::parse(Some("a".to_owned()), text).expect("the JSON is a command");
    assert_eq!(
        job.launcher,
        ["buck-out/run_test", "--time-limit", "20m", "--one-test"]
    );
    assert_eq!(job.command, ["buck-out/harness", "--exact", "tests::a::a"]);
    assert_eq!(
        job.environment,
        [
            ("BE_HOST".to_owned(), "buck-out/be-host".to_owned()),
            (
                "CARGO_MANIFEST_DIR".to_owned(),
                "crates/be-host-tests".to_owned()
            ),
        ]
    );
    assert_eq!(job.accepted(), Path::new("crates/be-host-tests/snapshots"));
    assert!(Job::parse(None, "{}").is_err());
}
