use super::*;
use block_plugin_api::{ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, Rect};

#[test]
fn a_click_outside_a_frame_child_hands_the_frame_back() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
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
            top_bar: block_plugin_api::TopBar::Hidden,
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Active,
            intrinsic: None,
            rotation: 0.0,
            opacity: 1.0,
            follows_view: false,
            dialog: None,
            docks: None,
            anchor: None,
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
