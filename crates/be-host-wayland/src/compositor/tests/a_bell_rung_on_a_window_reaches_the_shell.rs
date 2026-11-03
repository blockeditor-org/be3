use super::*;

#[test]
fn a_bell_rung_on_a_window_reaches_the_shell() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    let bell = harness.client.bell();
    assert_eq!(harness.compositor().windows().rung(id), 0);
    let revision = harness.compositor().windows().revision();

    bell.ring(Some(&window.surface));
    harness.settle();
    assert_eq!(harness.compositor().windows().rung(id), 1);
    assert!(
        harness.compositor().windows().revision() > revision,
        "the shell is told to read the window again"
    );

    bell.ring(None);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().rung(id),
        1,
        "a bell rung on no surface belongs to no window"
    );
}
