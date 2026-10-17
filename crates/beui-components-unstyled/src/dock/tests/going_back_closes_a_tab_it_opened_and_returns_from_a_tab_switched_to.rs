use super::*;

#[test]
fn going_back_closes_a_tab_it_opened_and_returns_from_a_tab_switched_to() {
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    assert_eq!(state.back(), None, "the first tab shown has no way back");

    state.push_to_focused(tab(2));
    state.push_to_focused(tab(3));
    state.show(tab(1));
    assert_eq!(
        state.back(),
        Some(DockVisit {
            from: tab(3),
            to: tab(1),
            opened: false,
        }),
        "a tab switched to goes back to the tab it was switched from"
    );

    state.go_back();
    assert_eq!(state.focused_tab(), Some(tab(3)));
    assert_eq!(
        state.back(),
        Some(DockVisit {
            from: tab(2),
            to: tab(3),
            opened: true,
        }),
        "going back undoes only the last visit, and a tab opened from another remembers it"
    );

    state.close(tab(3));
    assert_eq!(
        state.focused_tab(),
        Some(tab(2)),
        "closing the tab on show returns to the tab it was opened from"
    );
    assert_eq!(state.back().map(|visit| visit.from), Some(tab(1)));
    state.go_back();
    assert_eq!(state.focused_tab(), Some(tab(1)));
    assert_eq!(state.back(), None, "back runs out where it started");
}
