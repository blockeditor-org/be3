use super::*;

#[test]
fn dock_tabs_are_the_same_height_whether_or_not_they_close() {
    let closable = Func::new(|tab: unstyled::TabId| tab != unstyled::TabId::new(1));
    let dock = NodeRef::new();
    let built = dock.clone();
    let document = build(move || {
        let (state, set_state) = create_signal(unstyled::DockState::new([
            unstyled::TabId::new(1),
            unstyled::TabId::new(2),
        ]));
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                closable={closable}
                title={Func::new(|tab: unstyled::TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: unstyled::DockState| set_state.set(next)}
                on_close={move |_: unstyled::TabId| {}}
            >
                {move |tab: unstyled::TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());

    let fixed = tab_height(&harness, dock, "Tab 1");
    let closes = tab_height(&harness, dock, "Tab 2");
    assert_eq!(
        fixed, closes,
        "a tab with nothing to close is as tall as the one in the pane beside it, \
         which carries a close button"
    );
}

fn tab_height(harness: &Harness, dock: NodeId, title: &str) -> f32 {
    let label = dock_tab(harness.document(), dock, title);
    let mut node = label;
    while let Some(parent) = parent_of(harness.document(), dock, node) {
        if harness.document().node_kind(parent) == "click-catcher" {
            return harness.rect(parent).height();
        }
        node = parent;
    }
    panic!("the tab of {title} sits in a click catcher");
}

fn parent_of(document: &Document, root: NodeId, child: NodeId) -> Option<NodeId> {
    if document.children(root).contains(&child) {
        return Some(root);
    }
    document
        .children(root)
        .into_iter()
        .find_map(|node| parent_of(document, node, child))
}
