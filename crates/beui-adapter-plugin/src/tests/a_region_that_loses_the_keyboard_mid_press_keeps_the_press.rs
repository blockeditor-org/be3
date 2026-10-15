use beui_core::input::{PointerButton as Button, PointerPhase};
use block_editor_plugin::PointerButton;

use super::*;

struct Watching(
    Rc<std::cell::RefCell<Vec<Event>>>,
    Rc<std::cell::RefCell<Vec<PointerPhase>>>,
);

impl App for Watching {
    fn update(&mut self, context: &Context, _rect: Rect) {
        context.input(|input| {
            self.0.borrow_mut().extend(input.events.iter().cloned());
            self.1
                .borrow_mut()
                .extend(input.pointer_events.iter().map(|event| event.phase));
        });
    }
}

#[test]
fn a_region_that_loses_the_keyboard_mid_press_keeps_the_press() {
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let phases = Rc::new(std::cell::RefCell::new(Vec::new()));
    let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 60.0));
    let shown = region(whole, [80, 60]);
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Watching(events.clone(), phases.clone()),
    );
    surface.update(&shown);
    surface.input(&shown, &InputEvent::Focus(true));
    surface.input(&shown, &InputEvent::PointerMoved { x: 10.0, y: 10.0 });
    surface.input(
        &shown,
        &InputEvent::PointerButton {
            button: PointerButton::Primary,
            pressed: true,
            x: 10.0,
            y: 10.0,
        },
    );
    surface.update(&shown);
    surface.input(&shown, &InputEvent::Focus(false));
    surface.input(&shown, &InputEvent::PointerMoved { x: 40.0, y: 30.0 });
    surface.update(&shown);
    assert!(
        !phases
            .borrow()
            .iter()
            .any(|phase| matches!(phase, PointerPhase::Gone | PointerPhase::Cancel)),
        "the press the host still forwards goes on: {:?}",
        phases.borrow()
    );
    assert!(!events.borrow().contains(&Event::Focus(false)));

    surface.input(
        &shown,
        &InputEvent::PointerButton {
            button: PointerButton::Primary,
            pressed: false,
            x: 40.0,
            y: 30.0,
        },
    );
    surface.update(&shown);
    assert!(
        phases.borrow().contains(&PointerPhase::Up(Button::Primary)),
        "{:?}",
        phases.borrow()
    );
    assert!(
        events.borrow().contains(&Event::Focus(false)),
        "the lost keyboard is reported once the press is let go"
    );

    events.borrow_mut().clear();
    surface.input(&shown, &InputEvent::Focus(true));
    surface.input(
        &shown,
        &InputEvent::PointerButton {
            button: PointerButton::Primary,
            pressed: true,
            x: 10.0,
            y: 10.0,
        },
    );
    surface.input(&shown, &InputEvent::Focus(false));
    surface.input(&shown, &InputEvent::PointerLeft);
    surface.update(&shown);
    assert!(
        events.borrow().contains(&Event::Focus(false)),
        "a press the host stopped forwarding no longer holds the lost keyboard back"
    );
}
