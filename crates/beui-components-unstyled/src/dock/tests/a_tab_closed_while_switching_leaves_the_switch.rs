use super::*;

#[test]
fn a_tab_closed_while_switching_leaves_the_switch() {
    let mut state = DockState::new((1..=3).map(TabId::new));
    for tab in [1, 2, 3] {
        state.show(TabId::new(tab));
    }

    state.begin_switch(false);
    state.remove(TabId::new(2));

    let switch = state.switch().expect("the switch goes on");
    assert_eq!(switch.tabs, [TabId::new(3), TabId::new(1)]);
    assert_eq!(
        switch.chosen,
        TabId::new(1),
        "the choice moves on to the next tab still open"
    );
}
