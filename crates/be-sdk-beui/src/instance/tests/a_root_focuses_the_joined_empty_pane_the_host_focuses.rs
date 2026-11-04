use super::*;

#[test]
fn a_root_focuses_the_joined_empty_pane_the_host_focuses() {
    let (mut session, layout) = joined_then_away();

    session.child_focused(joined_body(&session, 9));
    show_docked(&mut session, WIDE, false);
    let pane = layout.state().with_untracked(|state| {
        state
            .focused_leaf()
            .and_then(|leaf| state.pane_key(leaf).map(str::to_owned))
    });
    assert!(
        pane.as_deref()
            .is_some_and(|pane| pane.ends_with("/editors")),
        "focus entering the empty pane's view focuses that pane, not {pane:?}"
    );
    assert_eq!(
        layout.focused(),
        Some(1),
        "the empty pane stands for tab 1, which its dock joins in place of"
    );
}
