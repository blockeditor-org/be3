use super::*;

#[test]
fn a_spliced_childs_bar_action_reaches_its_placer() {
    let mut docks = nested();
    let _ = updates(&mut docks, "shell");
    assert!(docks.bar(INNER, BarAction::Details));
    assert_eq!(
        docks.take_bars("outer"),
        [(OUTER, ViewId(2), INNER_CHILD, BarAction::Details)],
        "the frame that would carry it is not shown, so the host passes it on, to the tab view the child is placed in"
    );
    assert!(docks.take_bars("outer").is_empty());

    docks.heard("inner", vec![declare(INNER, None)]);
    assert!(
        !docks.bar(INNER, BarAction::Details),
        "a child that declares no dock is shown as a frame, which carries it"
    );
}
