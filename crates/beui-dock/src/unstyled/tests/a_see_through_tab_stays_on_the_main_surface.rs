use super::*;

#[test]
fn a_see_through_tab_stays_on_the_main_surface() {
    let mut state = DockState::default();
    state.reconcile(&DockSpec {
        see_through: vec![TabId::new(1)],
        ..spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("app", &[1]),
            pane("tools", &[2, 3]),
        ))
    });
    let window = DockDrop::Window {
        pos: Pos2::new(40.0, 40.0),
    };
    let [app, tools] = state.leaves(state.main())[..] else {
        panic!("the spec declares two panes");
    };

    assert!(state.shows_see_through(app));
    assert!(!state.shows_see_through(tools));
    assert!(!state.admits(Entry::Tab(TabId::new(1)), window));
    assert!(!state.admits_leaf(app, window));
    assert!(state.admits(
        Entry::Tab(TabId::new(1)),
        DockDrop::Tab {
            leaf: tools,
            index: usize::MAX
        }
    ));
    state.drop_tab(TabId::new(1), window);
    assert!(
        state.windows().is_empty(),
        "the see-through tab is not floated"
    );

    state.drop_tab(TabId::new(3), window);
    let floated = state.windows()[0];
    let inside = state.leaves(floated)[0];
    assert!(
        !state.admits(
            Entry::Tab(TabId::new(1)),
            DockDrop::Tab {
                leaf: inside,
                index: usize::MAX
            }
        ),
        "nor dropped into a window another tab floated into"
    );

    state.drop_tab(
        TabId::new(2),
        DockDrop::Tab {
            leaf: app,
            index: usize::MAX,
        },
    );
    assert!(
        !state.shows_see_through(app),
        "a pane shows through only while it shows the see-through tab"
    );
}
