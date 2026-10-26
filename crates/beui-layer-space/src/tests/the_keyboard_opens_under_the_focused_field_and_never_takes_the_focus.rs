use std::rc::Rc;

use beui_keyboard::styled::SoftwareKeyboard;
use beui_core::color::Color32;
use beui_core::context::WindowOutput;
use beui_core::display::Layer;
use beui_core::document::Document;
use beui_core::geometry::Rotation;
use beui_core::input::{ImeArea, Keyboard};
use beui_core::painter::{Corners, Shape};
use beui_macros::view;
use beui_view::reactive::enter;

use super::the_chrome_around_a_curved_panel::window;
use super::*;

const KEYBOARD: Vec2 = vec2(560.0, 210.0);

fn keyboard(context: &Context, id: WindowId) -> WindowOutput {
    let mut document = Document::new();
    let root = enter(&mut document, || {
        view! {
            <SoftwareKeyboard />
        }
    });
    document.set_root(root);
    let rect = Rect::from_min_size(Pos2::ZERO, KEYBOARD);
    let mut output = context.run(|context| document.show_content(context, rect));
    let mut shown = output.windows.remove(0);
    shown.id = id;
    shown.size = KEYBOARD;
    shown
}

fn typing_in(context: &Context, field: Rect) -> WindowOutput {
    let mut main = window(context, WindowId::MAIN, PLATFORM, "Typing");
    let mut layers = (*main.layers).clone();
    layers.push(Layer::Shape(Shape::Rect {
        rect: field,
        corner_radius: Corners::all(6.0),
        stroke_width: 0.0,
        color: Color32::WHITE,
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    }));
    main.layers = Rc::new(layers);
    main.window.requests.ime = Some(ImeArea {
        rect: field,
        cursor: Rect::from_min_size(field.min, vec2(2.0, field.height())),
        text: None,
        keyboard: Keyboard::Shown { asks: 1 },
    });
    main
}

fn open_keyboard(space: &mut Space, now: Instant, windows: Vec<WindowOutput>) -> WindowId {
    let window = WindowId::next();
    let mut output = app_frame(CursorIcon::Default);
    output.windows = windows;
    output.window_requests = vec![WindowRequest::Open {
        window,
        placement: WindowPlacement::AtPointer,
        size: KEYBOARD,
        role: WindowRole::Keyboard,
    }];
    space.frame(&output, &view(now));
    send(space, now, []);
    window
}

#[test]
fn the_keyboard_opens_under_the_focused_field_and_never_takes_the_focus() {
    let (mut space, now, context) = painted_space();
    assert_eq!(
        space.keyboard_attached(),
        None,
        "a mouse leaves the keyboard attached"
    );
    let field = Rect::from_min_size(pos2(240.0, 120.0), vec2(320.0, 40.0));
    let main_output = typing_in(&context, field);
    let id = open_keyboard(&mut space, now, vec![main_output.clone()]);
    assert_eq!(space.keyboards(), [id]);
    assert_eq!(
        space.focused(),
        Some(WindowId::MAIN),
        "opening the keyboard leaves the focus with the field"
    );
    let (main, panel) = (*main(&space), *space.panel(id).unwrap());
    let under = main.world(pos2(field.center().x, field.bottom() + crate::KEYBOARD_GAP));
    let top = panel.world(pos2(KEYBOARD.x / 2.0, 0.0));
    assert!(
        (top - under).length() < crate::OVER_NEARER + 1e-3,
        "its top edge sits just under the field, a little in front: {top:?} {under:?}"
    );
    assert_eq!(panel.metres_per_point, main.metres_per_point);

    let key = panel.world(pos2(KEYBOARD.x * 0.3, KEYBOARD.y * 0.6));
    drive(&mut space, now, [hand_pose(right_hand(), key)]);
    assert_eq!(
        space.keyboard_attached(),
        Some(false),
        "a laser counts as no keyboard attached"
    );
    let pressed = touches(&drive(&mut space, now, [trigger(Hand::Right, 1.0)]));
    assert_eq!(
        pressed
            .iter()
            .map(|(window, _, phase, _)| (*window, *phase))
            .collect::<Vec<_>>(),
        [(id, TouchPhase::Start)],
        "the laser touches the keyboard"
    );
    assert_eq!(space.focused(), Some(WindowId::MAIN), "and the focus stays");
    let windows = [main_output, keyboard(&context, id)];
    assert_scene(
        &mut space,
        now,
        &windows,
        "the_keyboard_opens_under_the_focused_field_and_never_takes_the_focus",
    );
    drive(&mut space, now, [trigger(Hand::Right, 0.0)]);

    let clicked = screen(&space, panel.world(pos2(40.0, 40.0)));
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(clicked),
            button(clicked, true),
            button(clicked, false),
        ],
    );
    assert_eq!(
        space.focused(),
        Some(WindowId::MAIN),
        "a click on the keyboard does not focus it either"
    );

    drive(&mut space, now, [Controller::Lost { hand: Hand::Right }]);
    assert_eq!(space.keyboard_attached(), None);
    space.close(id);
    assert_eq!(space.keyboards(), []);
}
