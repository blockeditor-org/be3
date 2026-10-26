use super::*;
use crate::dock::unstyled::DockApi;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn a_window_dragged_far_away_keeps_its_grip_in_the_dock() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(crate::reactive::Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    let window = floated_window(&mut harness, dock);
    let before = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window)
        .expect("the window has a rect");
    let bounds = harness.rect(dock);

    let bar = bounds.min + before.min.to_vec2() + vec2(before.width() - 80.0, 10.0);
    harness.drag(bar, bar + bounds.size() * 4.0);

    let away = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window)
        .expect("the window is still open");
    assert!(
        away.max.x > bounds.width() && away.max.y > bounds.height(),
        "a window can be dragged past the right and bottom edges: {away:?}"
    );
    assert!(
        away.min.x < bounds.width() - 80.0 && away.min.y < bounds.height() - 20.0,
        "the left of its bar stays inside the dock: {away:?}"
    );
    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    assert!(
        tab.min.x > bounds.min.x + away.min.x,
        "the window is drawn where it was left, with no stretch: {tab:?}"
    );

    let grip = bounds.min + away.min.to_vec2() + vec2(8.0, 10.0);
    harness.drag(grip, grip - vec2(300.0, 300.0));

    let back = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        back.min,
        away.min - vec2(300.0, 300.0),
        "the grip left inside the dock still drags the window"
    );

    let bar = bounds.min + back.min.to_vec2() + vec2(back.width() - 80.0, 10.0);
    harness.drag(bar, bar - bounds.size() * 4.0);

    let corner = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        corner.min,
        Pos2::ZERO,
        "a window stops at the left and top edges, where its grip would go out of reach"
    );
}
