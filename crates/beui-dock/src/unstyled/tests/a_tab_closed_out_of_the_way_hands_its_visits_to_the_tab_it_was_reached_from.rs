use super::*;

#[test]
fn a_tab_closed_out_of_the_way_hands_its_visits_to_the_tab_it_was_reached_from() {
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    let leaf = state.leaves(state.main())[0];
    state.split(leaf, Side::Right, 0.5, vec![tab(4)]);
    state.show(tab(1));
    state.push_to_focused(tab(2));
    state.push_to_focused(tab(3));

    state.close(tab(2));
    assert_eq!(state.focused_tab(), Some(tab(3)));
    assert_eq!(
        state.back(),
        Some(DockVisit {
            from: tab(1),
            to: tab(3),
            opened: true,
        }),
        "the tab opened from a closed one goes back to where the closed one came from"
    );

    state.show(tab(4));
    state.close(tab(4));
    assert_eq!(
        state.focused_tab(),
        Some(tab(3)),
        "closing the focused tab in a tiled dock shows the tab it was reached from"
    );
}
