use super::*;

#[test]
fn a_panic_before_the_end_of_the_log_is_still_reported() {
    let mut log = vec![
        "block-app: started".to_owned(),
        "block-app: thread 'be' panicked: panicked at crates/block-app/src/be.rs:1:1:".to_owned(),
        "the replica was gone".to_owned(),
        String::new(),
    ];
    log.extend((0..100).map(|frame| format!("  {frame}: a backtrace frame")));
    let report = log_report(&log.join("\n"));
    assert!(
        report.contains("panicked at crates/block-app/src/be.rs:1:1:\nthe replica was gone"),
        "the panic and its message are reported, got {report}"
    );
    assert!(
        !report.contains("block-app: started"),
        "only the panic is taken from before the tail"
    );
    assert!(report.ends_with("  99: a backtrace frame"));

    let short = log_report("one\ntwo");
    assert_eq!(short, "The end of the app's log:\none\ntwo");
}
