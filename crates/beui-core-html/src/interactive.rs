use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_tree::reactive::{Callback, ClickCallback};
use beui_types::Pasted;
use beui_types::input::{ImeEvent, KeyPress, PointerInput, PointerPress, WheelInput};
use beui_types::region::Area;

use crate::node::NodeId;

pub enum Never {}

pub struct ForwardedInput(Never);

#[derive(Default)]
pub struct Handlers {
    pub on_focus_change: Callback<bool>,
    pub on_activate: ClickCallback,
    pub on_key: Callback<KeyPress, bool>,
    pub on_text: Callback<String>,
    pub on_ime: Callback<ImeEvent>,
    pub on_paste: Callback<Pasted>,
    pub on_click: ClickCallback,
    pub on_click_at: Callback<PointerPress>,
    pub on_hover_change: Callback<bool>,
    pub on_hover_move: Callback<PointerPress>,
    pub on_active_change: Callback<bool>,
    pub on_cancel: ClickCallback,
    pub on_middle_click: ClickCallback,
    pub on_press: Callback<PointerPress>,
    pub on_secondary_press: Callback<PointerPress>,
    pub on_drag: Callback<PointerPress>,
    pub on_pointer: Callback<PointerInput, bool>,
    pub on_wheel: Callback<WheelInput, bool>,
}

#[derive(Default)]
pub struct Switches {
    pub press_focus: Cell<bool>,
    pub ime: Cell<bool>,
    pub hit_area: RefCell<Area>,
}

pub fn decorate(node: NodeId, handlers: Handlers, switches: Rc<Switches>) {
    #[cfg(target_arch = "wasm32")]
    web::decorate(node, handlers, switches);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (node, handlers, switches);
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use beui_types::input::{
        ImeEvent, Input, KeyPress, PointerButton, PointerId, PointerInput, PointerPress,
        PointerStage, ScrollGesture, TouchId, WheelInput, ZoomGesture, scrolled,
    };
    use beui_types::{Pasted, Pos2, Vec2, WindowId, pos2, vec2};
    use wasm_bindgen::JsCast;

    use super::{Handlers, Switches};
    use crate::dom::Listener;
    use crate::node::NodeId;

    struct State {
        node: NodeId,
        handlers: Handlers,
        switches: Rc<Switches>,
        pressed: Cell<Option<i32>>,
        pinch: RefCell<beui_web_input::wheel::Pinch>,
    }

    fn origin() -> Pos2 {
        crate::run::root()
            .and_then(crate::dom::dom)
            .and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
            .map(|root| {
                let rect = root.get_bounding_client_rect();
                pos2(rect.left() as f32, rect.top() as f32)
            })
            .unwrap_or(Pos2::ZERO)
    }

    struct Placed {
        pos: Pos2,
        local: Pos2,
        size: Vec2,
    }

    fn placed(element: &web_sys::Element, x: f64, y: f64) -> Placed {
        let rect = element.get_bounding_client_rect();
        let origin = origin();
        let client = pos2(x as f32, y as f32);
        Placed {
            pos: pos2(client.x - origin.x, client.y - origin.y),
            local: pos2(client.x - rect.left() as f32, client.y - rect.top() as f32),
            size: vec2(rect.width() as f32, rect.height() as f32),
        }
    }

    fn modifiers(event: &web_sys::MouseEvent) -> beui_types::Modifiers {
        beui_web_input::keys::modifiers(
            event.alt_key(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
        )
    }

    fn press(element: &web_sys::Element, event: &web_sys::MouseEvent) -> PointerPress {
        let at = placed(element, event.client_x().into(), event.client_y().into());
        let touch = event
            .dyn_ref::<web_sys::PointerEvent>()
            .is_some_and(|event| event.pointer_type() == "touch");
        PointerPress {
            window: WindowId::MAIN,
            pos: at.pos,
            local: at.local,
            size: at.size,
            fraction: vec2(
                if at.size.x > 0.0 {
                    at.local.x / at.size.x
                } else {
                    0.0
                },
                if at.size.y > 0.0 {
                    at.local.y / at.size.y
                } else {
                    0.0
                },
            ),
            clicks: u32::try_from(event.detail()).unwrap_or(1).max(1),
            modifiers: modifiers(event),
            touch,
            pen: None,
            predicted: None,
            time: crate::frame::now(),
        }
    }

    fn button(button: i16) -> Option<PointerButton> {
        match button {
            0 => Some(PointerButton::Primary),
            1 => Some(PointerButton::Middle),
            2 => Some(PointerButton::Secondary),
            3 => Some(PointerButton::Back),
            4 => Some(PointerButton::Forward),
            _ => None,
        }
    }

    fn pointer_input(
        element: &web_sys::Element,
        event: &web_sys::PointerEvent,
        stage: PointerStage,
    ) -> PointerInput {
        let at = placed(element, event.client_x().into(), event.client_y().into());
        let pointer = match event.pointer_type().as_str() {
            "touch" => PointerId::Touch(TouchId {
                device: 0,
                finger: event.pointer_id() as u64,
            }),
            "pen" => PointerId::Pen,
            _ => PointerId::Mouse,
        };
        PointerInput {
            pointer,
            button: button(event.button()),
            stage,
            pos: at.pos,
            local: at.local,
            size: at.size,
            scale: 1.0,
            inside: at.local.x >= 0.0
                && at.local.y >= 0.0
                && at.local.x <= at.size.x
                && at.local.y <= at.size.y,
            modifiers: modifiers(event),
            time: crate::frame::now(),
            settles: None,
        }
    }

    impl State {
        fn element(&self) -> Option<web_sys::Element> {
            crate::dom::dom(self.node).and_then(|dom| dom.dyn_into().ok())
        }

        fn hits(&self, element: &web_sys::Element, event: &web_sys::MouseEvent) -> bool {
            let at = placed(element, event.client_x().into(), event.client_y().into());
            match &*self.switches.hit_area.borrow() {
                beui_types::region::Area::Whole => true,
                beui_types::region::Area::Within(region) => region.contains(at.local),
                beui_types::region::Area::Nowhere | beui_types::region::Area::CaretHandles => false,
            }
        }

        fn pointer(&self, event: &web_sys::PointerEvent, stage: PointerStage) -> bool {
            if self.handlers.on_pointer.is_empty() {
                return false;
            }
            let Some(element) = self.element() else {
                return false;
            };
            self.handlers
                .on_pointer
                .call(pointer_input(&element, event, stage))
        }

        fn ime_events(&self, events: Vec<Input>) {
            for event in events {
                let Input::Ime(event) = event else {
                    continue;
                };
                if self.switches.ime.get() {
                    self.handlers.on_ime.call(event);
                } else if let ImeEvent::CommitText(text) = event {
                    self.handlers.on_text.call(text);
                }
            }
        }

        fn takes_text(&self) -> bool {
            self.switches.ime.get() || !self.handlers.on_text.is_empty()
        }
    }

    fn handled(event: &web_sys::Event, handled: bool) {
        if handled {
            event.prevent_default();
            event.stop_propagation();
        }
        reactive::flush();
    }

    fn attach(state: &Rc<State>, element: &web_sys::Element) -> Vec<Listener> {
        let target: &web_sys::EventTarget = element.as_ref();
        let mut listeners = Vec::new();
        let on = |name: &str, handler: Box<dyn Fn(web_sys::Event)>| {
            crate::dom::listen(target, name, false, move |event: web_sys::Event| {
                handler(event)
            })
        };
        let held = state.clone();
        listeners.push(on(
            "focusin",
            Box::new(move |event| {
                let Some(element) = held.element() else {
                    return;
                };
                let from = event
                    .dyn_ref::<web_sys::FocusEvent>()
                    .and_then(web_sys::FocusEvent::related_target)
                    .and_then(|from| from.dyn_into::<web_sys::Node>().ok());
                if from.is_some_and(|from| element.contains(Some(&from))) {
                    return;
                }
                held.handlers.on_focus_change.call(true);
                if held.takes_text() {
                    let sink = held.clone();
                    crate::ime::begin(held.node, Rc::new(move |events| sink.ime_events(events)));
                }
                reactive::flush();
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "focusout",
            Box::new(move |event| {
                let Some(element) = held.element() else {
                    return;
                };
                let to = event
                    .dyn_ref::<web_sys::FocusEvent>()
                    .and_then(web_sys::FocusEvent::related_target)
                    .and_then(|to| to.dyn_into::<web_sys::Node>().ok());
                if to.is_some_and(|to| element.contains(Some(&to))) {
                    return;
                }
                crate::ime::end(held.node);
                held.handlers.on_focus_change.call(false);
                reactive::flush();
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "keydown",
            Box::new(move |event| {
                let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                    return;
                };
                if event.is_composing() {
                    return;
                }
                let Some(key) = beui_web_input::keys::key(event) else {
                    return;
                };
                let modifiers = beui_web_input::keys::modifiers(
                    event.alt_key(),
                    event.ctrl_key(),
                    event.meta_key(),
                    event.shift_key(),
                );
                let mut done = held.handlers.on_key.call(KeyPress {
                    key,
                    pressed: true,
                    repeat: event.repeat(),
                    modifiers,
                });
                let activates = matches!(event.key().as_str(), "Enter" | " ");
                if !done && activates && !held.handlers.on_activate.is_empty() {
                    held.handlers.on_activate.call();
                    done = true;
                }
                handled(event, done);
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "keyup",
            Box::new(move |event| {
                let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                    return;
                };
                let Some(key) = beui_web_input::keys::key(event) else {
                    return;
                };
                let modifiers = beui_web_input::keys::modifiers(
                    event.alt_key(),
                    event.ctrl_key(),
                    event.meta_key(),
                    event.shift_key(),
                );
                let done = held.handlers.on_key.call(KeyPress {
                    key,
                    pressed: false,
                    repeat: false,
                    modifiers,
                });
                handled(event, done);
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "pointerdown",
            Box::new(move |event| {
                let (Some(event), Some(element)) =
                    (event.dyn_ref::<web_sys::PointerEvent>(), held.element())
                else {
                    return;
                };
                if !held.hits(&element, event) {
                    return;
                }
                if !held.switches.press_focus.get() {
                    event.prevent_default();
                }
                let mut done = held.pointer(event, PointerStage::Down);
                if event.button() == 0 {
                    held.pressed.set(Some(event.pointer_id()));
                    if !held.handlers.on_drag.is_empty() {
                        let _ = element.set_pointer_capture(event.pointer_id());
                    }
                    held.handlers.on_active_change.call(true);
                    if !held.handlers.on_press.is_empty() {
                        held.handlers.on_press.call(press(&element, event));
                        done = true;
                    }
                }
                if done {
                    event.stop_propagation();
                }
                reactive::flush();
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "pointermove",
            Box::new(move |event| {
                let (Some(event), Some(element)) =
                    (event.dyn_ref::<web_sys::PointerEvent>(), held.element())
                else {
                    return;
                };
                let done = held.pointer(event, PointerStage::Move);
                match held.pressed.get() {
                    Some(pointer) if pointer == event.pointer_id() => {
                        held.handlers.on_drag.call(press(&element, event));
                    }
                    _ => held.handlers.on_hover_move.call(press(&element, event)),
                }
                handled(event, done);
            }),
        ));
        for (name, stage) in [
            ("pointerup", PointerStage::Up),
            ("pointercancel", PointerStage::Cancel),
        ] {
            let held = state.clone();
            listeners.push(on(
                name,
                Box::new(move |event| {
                    let Some(event) = event.dyn_ref::<web_sys::PointerEvent>() else {
                        return;
                    };
                    let done = held.pointer(event, stage);
                    if held.pressed.get() == Some(event.pointer_id()) {
                        held.pressed.set(None);
                        held.handlers.on_active_change.call(false);
                        if stage == PointerStage::Cancel {
                            held.handlers.on_cancel.call();
                        }
                    }
                    handled(event, done);
                }),
            ));
        }
        for (name, hovered) in [("pointerenter", true), ("pointerleave", false)] {
            let held = state.clone();
            listeners.push(on(
                name,
                Box::new(move |_| {
                    held.handlers.on_hover_change.call(hovered);
                    reactive::flush();
                }),
            ));
        }
        let held = state.clone();
        listeners.push(on(
            "click",
            Box::new(move |event| {
                let (Some(event), Some(element)) =
                    (event.dyn_ref::<web_sys::MouseEvent>(), held.element())
                else {
                    return;
                };
                if !held.hits(&element, event) {
                    return;
                }
                let clicks =
                    !held.handlers.on_click.is_empty() || !held.handlers.on_click_at.is_empty();
                held.handlers.on_click.call();
                if !held.handlers.on_click_at.is_empty() {
                    held.handlers.on_click_at.call(press(&element, event));
                }
                if clicks {
                    event.stop_propagation();
                }
                reactive::flush();
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "auxclick",
            Box::new(move |event| {
                let Some(event) = event.dyn_ref::<web_sys::MouseEvent>() else {
                    return;
                };
                if event.button() == 1 && !held.handlers.on_middle_click.is_empty() {
                    held.handlers.on_middle_click.call();
                    handled(event, true);
                }
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "contextmenu",
            Box::new(move |event| {
                let (Some(event), Some(element)) =
                    (event.dyn_ref::<web_sys::MouseEvent>(), held.element())
                else {
                    return;
                };
                if held.handlers.on_secondary_press.is_empty() {
                    return;
                }
                held.handlers
                    .on_secondary_press
                    .call(press(&element, event));
                handled(event, true);
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "wheel",
            Box::new(move |event| {
                let (Some(event), Some(element)) =
                    (event.dyn_ref::<web_sys::WheelEvent>(), held.element())
                else {
                    return;
                };
                if held.handlers.on_wheel.is_empty() {
                    return;
                }
                let delta = vec2(event.delta_x() as f32, event.delta_y() as f32);
                let input = beui_web_input::wheel::wheel(
                    delta,
                    event.delta_mode(),
                    event.ctrl_key(),
                    &held.pinch.borrow(),
                );
                let at = placed(&element, event.client_x().into(), event.client_y().into());
                let wheel = match input {
                    Some(Input::Scroll(delta)) => WheelInput::Scroll(ScrollGesture {
                        delta: scrolled(delta, 1.0),
                        fling: Vec2::ZERO,
                        pos: at.pos,
                        local: at.local,
                        size: at.size,
                        modifiers: modifiers(event),
                        held: false,
                    }),
                    Some(Input::Zoom(factor)) => WheelInput::Zoom(ZoomGesture {
                        factor,
                        pos: at.pos,
                        local: at.local,
                        size: at.size,
                    }),
                    _ => return,
                };
                let done = held.handlers.on_wheel.call(wheel);
                handled(event, done);
            }),
        ));
        let held = state.clone();
        listeners.push(on(
            "paste",
            Box::new(move |event| {
                if held.handlers.on_paste.is_empty() {
                    return;
                }
                let text = event
                    .dyn_ref::<web_sys::ClipboardEvent>()
                    .and_then(web_sys::ClipboardEvent::clipboard_data)
                    .and_then(|data| data.get_data("text/plain").ok());
                if let Some(text) = text {
                    held.handlers.on_paste.call(Pasted::Text(text));
                    handled(&event, true);
                }
            }),
        ));
        listeners
    }

    pub(super) fn decorate(node: NodeId, handlers: Handlers, switches: Rc<Switches>) {
        let state = Rc::new(State {
            node,
            handlers,
            switches,
            pressed: Cell::new(None),
            pinch: RefCell::default(),
        });
        let attached: RefCell<Option<(web_sys::Element, Vec<Listener>)>> = RefCell::default();
        crate::frame::on_layout(Rc::new(move || {
            let Some(element) = state.element() else {
                return false;
            };
            let mut attached = attached.borrow_mut();
            if attached.as_ref().is_some_and(|(held, _)| *held == element) {
                return false;
            }
            let listeners = attach(&state, &element);
            *attached = Some((element, listeners));
            false
        }));
    }
}
