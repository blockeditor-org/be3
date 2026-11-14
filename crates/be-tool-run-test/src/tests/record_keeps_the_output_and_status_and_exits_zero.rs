use super::{options, run_ok, scratch, shell};

#[test]
fn record_keeps_the_output_and_status_and_exits_zero() {
    let directory = scratch("record");
    let out = directory.join("out");
    let mut options = options(shell(
        "echo out; echo err >&2; touch \"$MADE_PAINTINGS/p\"; exit 5",
    ));
    options.record = Some(out.clone());
    assert_eq!(run_ok(&options), 0);
    assert_eq!(std::fs::read_to_string(out.join("status")).unwrap(), "5\n");
    let log = std::fs::read_to_string(out.join("log")).unwrap();
    let (output, usage) = log.split_once("run-test: ").unwrap();
    assert_eq!(output, "out
err
");
    assert!(usage.starts_with("the test used "), "{log}");
    assert!(out.join("made/p").is_file());
    let mut missing = super::options(vec!["/nonexistent/test".into()]);
    missing.record = Some(directory.join("missing"));
    assert_eq!(run_ok(&missing), 0);
    assert_eq!(
        std::fs::read_to_string(directory.join("missing/status")).unwrap(),
        "127\n"
    );
}
