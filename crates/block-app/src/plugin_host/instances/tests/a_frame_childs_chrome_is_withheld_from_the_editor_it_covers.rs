use super::*;
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, ChildRect,
};

const CHILD: ChildId = ChildId(1);
const SIDEBAR: Rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(60.0, 110.0));

fn active_child(instances: &mut Instances) {
    instances.set_children(ChildPlacements {
        instance: INSTANCE,
        region: REGION,
        generation: 1,
        children: vec![ChildPlacement {
            child: CHILD,
            block_id: [0; 16],
            block_type: [0; 16],
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
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Active,
            intrinsic: None,
            rotation: 0.0,
            opacity: 1.0,
        }],
        occluders: Vec::new(),
    });
}

fn press(at: Pos2) {
    host::register(
        TARGET,
        Rect::from_min_size(pos2(10.0, 10.0), SIZE),
        Rect::EVERYTHING,
        0,
    );
    host::test_frame(
        vec![
            beui::Event::PointerMoved(at),
            beui::Event::PointerButton {
                pos: at,
                button: beui::PointerButton::Primary,
                pressed: true,
                modifiers: beui::Modifiers::NONE,
            },
        ],
        Some(at),
        true,
    );
}

fn still_active(instances: &Instances) -> bool {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect);
    children.iter().any(|child| child.is_active())
}

fn pressed(messages: &[Message]) -> bool {
    messages.iter().any(|message| match message {
        Message::Input(batch) => batch
            .events
            .iter()
            .any(|event| matches!(event, InputEvent::PointerButton { pressed: true, .. })),
        _ => false,
    })
}

#[test]
fn a_frame_childs_chrome_is_withheld_from_the_editor_it_covers() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    active_child(&mut instances);
    let overlay = FrameOverlay {
        owner: Some(EditorInstanceId(INSTANCE.0 + 1)),
        rects: vec![SIDEBAR],
    };

    press(pos2(20.0, 80.0));
    let messages = instances.frame_input(PASS, &overlay);

    assert!(!pressed(&messages));
    assert!(still_active(&instances));

    press(pos2(80.0, 80.0));
    let messages = instances.frame_input(PASS, &overlay);

    assert!(pressed(&messages));
    assert!(!still_active(&instances));
}
