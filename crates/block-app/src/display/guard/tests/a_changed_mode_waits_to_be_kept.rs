use super::*;

#[test]
fn a_changed_mode_waits_to_be_kept() {
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    assert!(!guard.asking());

    assert!(
        !guard.take(saved(Some(small()))),
        "a new mode is not saved until it is kept"
    );
    assert!(guard.asking());

    assert_eq!(guard.keep(), Vec::new(), "keeping needs no further edit");
    assert!(!guard.asking());
    assert_eq!(guard.applied().mode(MONITOR), Some(small()));
}
