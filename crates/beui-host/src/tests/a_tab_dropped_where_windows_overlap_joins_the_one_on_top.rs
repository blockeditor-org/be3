use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_tab_dropped_where_windows_overlap_joins_the_one_on_top() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(4);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    for (title, to) in [("Tab 2", pos2(300.0, 150.0)), ("Tab 3", pos2(450.0, 200.0))] {
        let tab = harness.center(dock_tab(harness.document(), dock, title));
        drag_with(&mut harness, tab, to, Modifiers::ALT);
        harness.frame(Vec::new());
    }
    let state = harness.document().api::<DockApi>(dock).state();
    let [below, above] = [state.windows()[0], state.windows()[1]];
    let layer = |harness: &Harness, title: &str| {
        let document = harness.document();
        let layers = document.layers();
        document
            .layer_of(&layers, dock_tab(document, dock, title))
            .expect("the tab is in a layer")
    };
    assert!(layer(&harness, "Tab 3") > layer(&harness, "Tab 2"));
    let overlap = pos2(520.0, 360.0);

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    harness.drag(tab, overlap);
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    assert!(
        state.surface_tabs(above).contains(&TabId::new(1)),
        "the drop goes to the window drawn on top: {:?}",
        state.surface_tabs(above)
    );

    harness.click(pos2(250.0, 400.0));
    harness.frame(Vec::new());
    assert!(
        layer(&harness, "Tab 2") > layer(&harness, "Tab 3"),
        "a press on the window beneath raises it"
    );
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 4"));
    harness.drag(tab, overlap);
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    assert!(
        state.surface_tabs(below).contains(&TabId::new(4)),
        "and the next drop there goes to it, as the layers stack: {:?}",
        state.surface_tabs(below)
    );
}
