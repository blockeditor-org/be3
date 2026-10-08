use super::*;

#[test]
fn reverting_a_change_restores_the_mode_before_it() {
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    guard.take(saved(Some(small())));
    assert!(
        !guard.take(saved(Some(fast()))),
        "a second change before the first is answered still waits"
    );

    let edits = guard.revert();
    assert_eq!(
        edits,
        vec![DisplaySettings::set_mode(MONITOR, Some(slow()))],
        "the block is put back to the mode that was showing before either change"
    );
    assert_eq!(guard.applied(), &saved(Some(slow())));
    assert!(!guard.asking());

    guard.take(saved(Some(small())));
    guard.take(saved(Some(slow())));
    assert!(
        !guard.asking(),
        "changing back before answering needs no answer"
    );
}
