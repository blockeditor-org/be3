use super::*;

fn panic_with_backtrace(message: &str, frames: usize) -> Vec<String> {
    let mut lines = vec![
        "block-app: thread 'main' panicked: panicked at crates/beui-core/src/node.rs:1:1:"
            .to_owned(),
        message.to_owned(),
        String::new(),
    ];
    for frame in 0..frames {
        lines.push(format!("  {frame:>2}: frame_{frame}"));
        lines.push(format!("             at ./src/frame_{frame}.rs:1:1"));
    }
    lines
}

#[test]
fn a_panic_is_reported_with_its_whole_backtrace() {
    let mut log = vec!["block-app: started".to_owned()];
    log.extend(panic_with_backtrace("node was removed", 60));
    let report = log_report(&log.join("\n"));
    assert!(
        report.contains("panicked at crates/beui-core/src/node.rs:1:1:\nnode was removed"),
        "a panic at the end of the log is reported from its first line, got {report}"
    );
    assert!(report.contains("   0: frame_0\n"));
    assert!(report.ends_with("at ./src/frame_59.rs:1:1"));
    assert!(!report.contains("block-app: started"));

    let mut log = panic_with_backtrace("the replica was gone", 60);
    log.extend((0..50).map(|line| format!("block-app: later line {line}")));
    let report = log_report(&log.join("\n"));
    let (before, end) = report
        .split_once("The end of the app's log:\n")
        .expect("the report has the log's end");
    assert!(
        before.contains("the replica was gone\n\n   0: frame_0\n")
            && before.contains("at ./src/frame_59.rs:1:1"),
        "a panic before the end keeps its whole backtrace, got {before}"
    );
    assert!(!before.contains("later line"));
    assert!(end.starts_with("block-app: later line 10\n"));
}
