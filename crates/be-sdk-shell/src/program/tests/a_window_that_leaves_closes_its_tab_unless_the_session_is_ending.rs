use super::*;

#[test]
fn a_window_that_leaves_closes_its_tab_unless_the_session_is_ending() {
    let mut programs = Programs::new();
    let mut next = 2;
    place(
        &mut programs,
        &[window(1, "foot", None), window(2, "kate", None)],
        false,
        &mut next,
        |_| true,
    );
    assert_eq!(bound(&programs), vec![(2, Some(1)), (3, Some(2))]);

    let gone = place(
        &mut programs,
        &[window(2, "kate", None)],
        false,
        &mut next,
        |_| true,
    );
    assert_eq!(gone, vec![TabId::new(2)]);
    assert_eq!(bound(&programs), vec![(3, Some(2))]);

    let gone = place(&mut programs, &[], true, &mut next, |record| {
        record.program_id != "kate"
    });
    assert_eq!(
        gone,
        vec![TabId::new(3)],
        "a window with no way to start it again leaves no placeholder"
    );
    place(
        &mut programs,
        &[window(4, "foot", None)],
        false,
        &mut next,
        |_| true,
    );
    let gone = place(&mut programs, &[], true, &mut next, |_| true);
    assert!(gone.is_empty());
    assert_eq!(
        bound(&programs),
        vec![(4, None)],
        "while the session ends, a window that closes leaves a placeholder"
    );
}
