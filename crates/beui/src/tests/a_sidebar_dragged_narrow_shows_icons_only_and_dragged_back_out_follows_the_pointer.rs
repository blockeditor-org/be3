use super::*;
use crate::unstyled::{DockApi, SIDEBAR_WIDTH};
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
fn a_sidebar_dragged_narrow_shows_icons_only_and_dragged_back_out_follows_the_pointer() {
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

    let edge = pos2(wide - 3.0, 300.0);
    harness.frame(vec![Input::PointerMoved(edge)]);
    harness.frame(vec![Input::PointerButton {
        pos: edge,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    let narrowed = edge - vec2(SIDEBAR_WIDTH - 10.0, 0.0);
    harness.frame(vec![Input::PointerMoved(narrowed)]);
    assert!(
        icons_only(&harness),
        "dragging the edge past the narrowest a sidebar goes shows only icons"
    );
    after(&mut harness, Duration::from_millis(40), None);
    let moving = panel_left(&harness);
    assert!(
        moving < wide && moving > narrowed.x,
        "the sidebar slides narrower rather than jumping: {moving}"
    );
    after(&mut harness, Duration::from_secs(1), None);
    let narrow = panel_left(&harness);
    assert!(
        narrow < moving && narrow > narrowed.x,
        "and settles as wide as an icon, not at the pointer: {narrow}"
    );
    assert!(text_within(harness.document(), dock, "Tab 1").is_none());

    let out = edge - vec2(30.0, 0.0);
    after(&mut harness, Duration::from_millis(16), Some(out));
    assert!(
        !icons_only(&harness),
        "dragging back out, still holding the edge, shows the titles again"
    );
    after(&mut harness, Duration::from_millis(40), None);
    let growing = panel_left(&harness);
    assert!(
        growing > narrow && growing < out.x + 3.0,
        "the sidebar slides out towards the pointer: {growing}"
    );
    let further = out + vec2(10.0, 0.0);
    after(&mut harness, Duration::from_millis(16), Some(further));
    after(&mut harness, Duration::from_secs(1), None);
    assert_eq!(
        panel_left(&harness),
        further.x + 3.0,
        "and catches up with the pointer, following it as it goes on moving"
    );
    harness.frame(vec![Input::PointerButton {
        pos: further,
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
        SIDEBAR_WIDTH - 20.0,
        "the sidebar keeps the width it was let go at"
    );
}
