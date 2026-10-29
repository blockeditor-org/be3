use super::*;
use crate::dock::unstyled::DockApi;

fn choose(harness: &mut Harness, dock: NodeId, pos: Pos2, item: &str) {
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());
    let row = text_within(harness.document(), dock, item)
        .unwrap_or_else(|| panic!("the grip's menu offers {item}"));
    harness.click(harness.center(row));
    harness.frame(Vec::new());
}

#[test]
fn dock_tabs_shown_as_icons_only_leave_out_their_titles() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let label = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    let grip = pos2(label.left() - 25.0, label.center().y);
    let panel = harness.rect(harness.find("content.1"));

    choose(&mut harness, dock, grip, "Icons only");
    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    assert!(
        state.is_icons_only(leaf),
        "the pane shows its tabs as icons"
    );
    for title in ["Tab 1", "Tab 2", "Tab 3"] {
        assert!(
            text_within(harness.document(), dock, title).is_none(),
            "{title} is not written in its tab"
        );
    }
    assert_eq!(
        harness.rect(harness.find("content.1")),
        panel,
        "the bar stays across the top, as tall as before"
    );

    choose(&mut harness, dock, grip, "Icons only");
    assert!(
        !harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .is_icons_only(leaf),
        "choosing it again shows the titles"
    );
    assert!(text_within(harness.document(), dock, "Tab 2").is_some());

    choose(&mut harness, dock, grip, "Close all tabs");
    assert!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .all_tabs()
            .is_empty(),
        "the grip's menu closes every tab in the pane"
    );
}
