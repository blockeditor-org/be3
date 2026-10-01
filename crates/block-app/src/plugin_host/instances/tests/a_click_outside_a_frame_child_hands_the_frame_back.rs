use super::*;
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, ChildRect,
};

#[test]
fn a_click_outside_a_frame_child_hands_the_frame_back() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    instances.set_children(ChildPlacements {
        instance: INSTANCE,
        region: REGION,
        generation: 1,
        size: block_plugin_api::Size {
            width: SIZE.x,
            height: SIZE.y,
        },
        children: vec![ChildPlacement {
            child: ChildId(1),
            block_id: [1; 16],
            block_type: [0; 16],
            view_block: None,
            rect: ChildRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            clip: ChildRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            own_frame: false,
            top_bar: block_plugin_api::TopBar::Hidden,
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Active,
            intrinsic: None,
            rotation: 0.0,
            opacity: 1.0,
        }],
        occluders: Vec::new(),
    });
    assert!(
        instances.frame_child(INSTANCE).is_some(),
        "an active child owns the frame"
    );

    instances.revoke_active(INSTANCE, REGION);

    assert!(
        instances.frame_child(INSTANCE).is_none(),
        "once a click outside revokes it, the editor holding it gets its chrome back at once"
    );
}
