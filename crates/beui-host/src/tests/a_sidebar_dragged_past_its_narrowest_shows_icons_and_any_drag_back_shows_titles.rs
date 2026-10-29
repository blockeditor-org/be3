use super::*;
use crate::dock::unstyled::{DockApi, MIN_SIDEBAR_WIDTH, SIDEBAR_WIDTH};
use std::time::Duration;

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

fn panel_left(harness: &Harness) -> f32 {
    harness.rect(harness.find("content.1")).left()
}

fn after(harness: &mut Harness, by: Duration, to: Option<Pos2>) {
    harness.advance(by);
    harness.frame(to.map(Input::PointerMoved).into_iter().collect());
}

#[test]
fn a_sidebar_dragged_past_its_narrowest_shows_icons_and_any_drag_back_shows_titles() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let label = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    let grip = pos2(label.left() - 25.0, label.center().y);
    choose(&mut harness, dock, grip, "Show tabs in a sidebar");
    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    let wide = panel_left(&harness);
    let icons_only = |harness: &Harness| {
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .is_icons_only(leaf)
    };
    let shrink = SIDEBAR_WIDTH - MIN_SIDEBAR_WIDTH;

    let edge = pos2(wide - 3.0, 300.0);
    harness.frame(vec![Input::PointerMoved(edge)]);
    harness.frame(vec![Input::PointerButton {
        pos: edge,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    after(
        &mut harness,
        Duration::from_millis(16),
        Some(edge - vec2(shrink, 0.0)),
    );
    assert!(
        !icons_only(&harness),
        "at its narrowest the sidebar keeps its titles"
    );
    assert_eq!(panel_left(&harness), wide - shrink);

    after(
        &mut harness,
        Duration::from_millis(16),
        Some(edge - vec2(shrink + 1.0, 0.0)),
    );
    assert!(
        icons_only(&harness),
        "a point further left shows only icons"
    );
    after(&mut harness, Duration::from_millis(40), None);
    let moving = panel_left(&harness);
    assert!(
        moving < wide - shrink,
        "the sidebar slides narrower rather than jumping: {moving}"
    );
    let far = edge - vec2(SIDEBAR_WIDTH - 10.0, 0.0);
    after(&mut harness, Duration::from_secs(1), Some(far));
    let narrow = panel_left(&harness);
    assert!(
        narrow < moving && narrow > far.x + 3.0,
        "and settles as wide as an icon, not at the pointer: {narrow}"
    );
    assert!(text_within(harness.document(), dock, "Tab 1").is_none());

    let back = far + vec2(5.0, 0.0);
    after(&mut harness, Duration::from_millis(16), Some(back));
    assert!(
        !icons_only(&harness),
        "any drag to the right shows the titles again, wherever the pointer is"
    );
    after(&mut harness, Duration::from_secs(1), None);
    assert_eq!(
        panel_left(&harness),
        wide - shrink,
        "at the narrowest width that fits them, well right of the pointer"
    );

    let out = edge - vec2(60.0, 0.0);
    after(&mut harness, Duration::from_millis(16), Some(out));
    after(&mut harness, Duration::from_secs(1), None);
    assert_eq!(
        panel_left(&harness),
        out.x + 3.0,
        "once the pointer passes that edge, the sidebar grows with it"
    );
    harness.frame(vec![Input::PointerButton {
        pos: out,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .sidebar_width(leaf),
        SIDEBAR_WIDTH - 60.0,
        "the sidebar keeps the width it was let go at"
    );
}
