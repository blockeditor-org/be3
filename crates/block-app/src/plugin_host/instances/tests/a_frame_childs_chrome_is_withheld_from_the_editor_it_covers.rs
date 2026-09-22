use super::*;
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, ChildRect,
};

const CHILD: ChildId = ChildId(1);
const SIDEBAR: egui::Rect =
    egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(60.0, 110.0));

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

fn press(context: &egui::Context, id: egui::Id, at: egui::Pos2) {
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), SIZE);
    let input = egui::RawInput {
        events: vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..egui::RawInput::default()
    };
    let _ = context.run_ui(input, |ui| {
        ui.interact(rect, id, egui::Sense::click_and_drag());
    });
}

fn still_active(instances: &Instances) -> bool {
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), SIZE);
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
    let (mut instances, context, id) = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    active_child(&mut instances);
    let overlay = FrameOverlay {
        owner: Some(EditorInstanceId(INSTANCE.0 + 1)),
        rects: vec![SIDEBAR],
    };

    press(&context, id, egui::pos2(20.0, 80.0));
    let messages = instances.frame_input(&context, PASS, &overlay);

    assert!(!pressed(&messages));
    assert!(still_active(&instances));

    press(&context, id, egui::pos2(80.0, 80.0));
    let messages = instances.frame_input(&context, PASS, &overlay);

    assert!(pressed(&messages));
    assert!(!still_active(&instances));
}
