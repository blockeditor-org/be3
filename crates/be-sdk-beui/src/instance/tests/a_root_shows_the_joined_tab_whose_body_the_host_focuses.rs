use super::*;

#[test]
fn a_root_shows_the_joined_tab_whose_body_the_host_focuses() {
    let (mut session, layout) = joined_then_away();

    session.child_focused(joined_body(&session, 5));
    show_docked(&mut session, WIDE, false);
    assert_eq!(
        layout.focused(),
        Some(1),
        "focus entering the joined tab's body shows that tab, which stands for tab 1"
    );
}
