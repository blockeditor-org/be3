use super::*;
use be_sdk_api::{ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, Rect};

#[test]
fn an_escape_in_a_placer_hands_the_frame_back_and_keeps_its_child_active() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.announce(screens);
    instances.set_children(ChildPlacements {
        instance: INSTANCE,
        view: REGION,
        children: vec![ChildPlacement {
            child: ChildId(1),
            content: ChildContent::Block {
                block_id: [1; 16],
                block_type: [0; 16],
                view_block: None,
                editor: None,
            },
            rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(20.0, 20.0)),
            clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 100.0)),
            own_frame: false,
            top_bar: be_sdk_api::TopBar::Hidden,
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Active,
            intrinsic: None,
            fit: None,
            rotation: 0.0,
            opacity: 1.0,
            follows_view: false,
            dialog: None,
            docks: None,
            anchor: None,
            fullscreen: false,
            keyboard_ask: 0,
        }],
        occluders: Vec::new(),
    });
    assert!(
        instances.frame_child(INSTANCE).is_some(),
        "an active child owns the frame"
    );

    // An Escape the placer does not handle hands back the frame, as under a root.
    assert!(instances.revoke_frame_children(INSTANCE, REGION));
    assert!(
        instances.frame_child(INSTANCE).is_none(),
        "the editor holding the child gets its chrome back"
    );
    let screen = instances
        .entries
        .get(&INSTANCE)
        .and_then(|entry| entry.screens.get(&REGION))
        .expect("the placer's view is shown");
    assert!(
        screen.revoked.is_empty(),
        "the child stays active: the keyboard stays where it is"
    );
    assert!(
        !instances.revoke_frame_children(INSTANCE, REGION),
        "a second Escape has no frame to hand back"
    );
}
