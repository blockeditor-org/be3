use super::*;
use crate::unstyled::{DockState, DockTree, DockTreeEntry, SIDEBAR_WIDTH, TabId};

fn shown(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some_and(|rect| rect.width() > 0.0 && rect.height() > 0.0)
}

#[test]
fn a_drop_the_dock_would_refuse_draws_no_drop_marker() {
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let mut layout = DockState::new([TabId::new(1)]);
        let leaf = layout.leaves(layout.main())[0];
        layout.insert_pinned_group(
            leaf,
            1,
            &DockTree::Tabs {
                entries: vec![
                    DockTreeEntry::Tab(TabId::new(10)),
                    DockTreeEntry::Tab(TabId::new(11)),
                ],
                active: 0,
                vertical: false,
                sidebar: SIDEBAR_WIDTH,
            },
        );
        let (state, set_state) = create_signal(layout);
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_state.set(next)}
                on_close={move |_: TabId| {}}
            >
                {move |tab: TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
        }
    });
    dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let root = harness.document().root().expect("the dock was built");
    let pinned = harness.center(dock_tab(harness.document(), root, "Tab 10"));
    let outside = harness.center(dock_tab(harness.document(), root, "Tab 1"));
    let inside = harness.center(dock_tab(harness.document(), root, "Tab 11"));

    harness.press_at(pinned);
    harness.frame(vec![Event::PointerMoved(outside)]);
    harness.frame(Vec::new());

    assert!(
        !shown(&harness, "dock.drop"),
        "a pinned tab over a bar outside its group is shown no place to land"
    );

    harness.frame(vec![Event::PointerMoved(inside)]);
    harness.frame(Vec::new());

    assert!(
        shown(&harness, "dock.drop"),
        "the same tab over its own group's bar is shown where it would land"
    );
    harness.release_at(inside);
}
