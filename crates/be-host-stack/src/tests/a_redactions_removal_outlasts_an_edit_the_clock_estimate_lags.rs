/// A redacted log document's removal is as of the server's clock now, or just after what the
/// index holds it at when that is later, as when the clock's estimate lags the arrival of the
/// edit that put it: a removal at or before it would lose, and the text would stay found.
#[test]
fn a_redactions_removal_outlasts_an_edit_the_clock_estimate_lags() {
    let held = |time: i64| be_lib_search::Commit { time, id: [7; 32] };
    let removal = |kept, now| crate::search::redaction_commit(kept, "log.4", now);
    assert_eq!(removal(None, 1_000).time, 1_000);
    assert_eq!(removal(Some(held(500)), 1_000).time, 1_000);
    let lagging = removal(Some(held(2_000)), 1_000);
    assert_eq!(
        lagging.time, 2_001,
        "the edit arrived after the clock's estimate"
    );
    assert!(
        lagging > held(2_000),
        "the removal wins over what the index holds"
    );
}
