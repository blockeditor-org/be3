use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_tree::reactive::{Callback, ClickCallback};
use beui_types::input::{ImeEvent, KeyPress, PointerInput, PointerPress, WheelInput};
use beui_types::region::{Area, Claim};
use beui_types::{Pasted, Pos2, SoftKeyboard, Vec2};

use crate::node::NodeId;

pub use beui_types::forward::ForwardedInput;

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
    pub on_forward: Callback<ForwardedInput>,
    pub on_wheel: Callback<WheelInput, bool>,
    pub on_activate_change: Callback<bool>,
    pub on_step: Callback<f32>,
    pub on_motion: Callback<Vec2>,
    pub on_press_outside: Callback<Pos2>,
}

#[derive(Default)]
pub struct Switches {
    pub press_focus: Cell<bool>,
    pub ime: Cell<bool>,
    pub hit_area: RefCell<Area>,
    pub capture_presses: RefCell<Area>,
    pub claims: RefCell<Vec<Claim>>,
    pub pointer_locked: Cell<bool>,
    pub every_sample: Cell<bool>,
    pub grabs_pointer: Cell<bool>,
    pub repeat_drag: Cell<bool>,
    pub keyboard: Cell<SoftKeyboard>,
}

pub fn covers(area: &Area, local: Pos2) -> bool {
    match area {
        Area::Whole => true,
        Area::Within(region) => region.contains(local),
        Area::Nowhere | Area::CaretHandles => false,
    }
}

pub fn lock_pointer(node: NodeId, locked: bool) {
    #[cfg(target_arch = "wasm32")]
    web::lock_pointer(node, locked);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (node, locked);
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
        ImeEvent, Input, KeyPress, PointerButton, PointerId, PointerInput, PointerPhase,
        PointerPress, PointerStage, ScrollGesture, TouchId, WheelInput, ZoomGesture, scrolled,
    };
    use beui_types::{Pasted, Pos2, Rect, Vec2, WindowId, pos2, vec2};
    use wasm_bindgen::JsCast;

    use super::{Handlers, Switches};
    use crate::dom::Listener;
    use crate::node::NodeId;

    struct State {
        node: NodeId,
        handlers: Handlers,
        switches: Rc<Switches>,
        pressed: Cell<Option<i32>>,
        held: Cell<Option<(i32, PointerButton)>>,
        pinch: RefCell<beui_web_input::wheel::Pinch>,
        composed: RefCell<String>,
        last_press: RefCell<Option<PointerPress>>,
        activating: Cell<bool>,
        repeating: Cell<Option<crate::frame::Timer>>,
        release: RefCell<Vec<Listener>>,
    }

    const CAPTURED: &str = "beuiCaptured";

    fn captured(event: &web_sys::PointerEvent) -> bool {
        js_sys::Reflect::get(event, &CAPTURED.into()).is_ok_and(|held| held.is_truthy())
    }

    const REPEAT: std::time::Duration = std::time::Duration::from_millis(16);

    pub(super) fn lock_pointer(node: NodeId, locked: bool) {
        let (Some(element), Some(document)) = (crate::dom::element(node), crate::dom::document())
        else {
            return;
        };
        let holds = document.pointer_lock_element().as_ref() == Some(&element);
        match (locked, holds) {
            (true, false) => element.request_pointer_lock(),
            (false, true) => document.exit_pointer_lock(),
            _ => {}
        }
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
        crate::page::note_pointer(touch);
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
            clicks: clicks(event, at.pos),
            modifiers: modifiers(event),
            touch,
            pen: None,
            predicted: None,
            time: crate::frame::now(),
        }
    }

    thread_local! {
        static CLICKS: RefCell<(beui_types::input::ClickCounter, Option<f64>)> =
            RefCell::default();
    }

    fn clicks(event: &web_sys::MouseEvent, pos: Pos2) -> u32 {
        CLICKS.with(|clicks| {
            let (counter, counted) = &mut *clicks.borrow_mut();
            let stamp = event.time_stamp();
            if event.type_() == "pointerdown" && event.button() == 0 && *counted != Some(stamp) {
                *counted = Some(stamp);
                counter.count(pos, crate::frame::now());
            }
            counter.clicks().max(1)
        })
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
            unclaimed: false,
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

        fn forward(&self, event: &web_sys::PointerEvent, stage: PointerStage) -> bool {
            if self.handlers.on_forward.is_empty() {
                return false;
            }
            let Some(element) = self.element() else {
                return false;
            };
            let input = pointer_input(&element, event, stage);
            let button = input.button.unwrap_or(PointerButton::Primary);
            let phase = match stage {
                PointerStage::Down => {
                    let _ = element.set_pointer_capture(event.pointer_id());
                    PointerPhase::Down(button)
                }
                PointerStage::Move => PointerPhase::Move,
                PointerStage::Up => PointerPhase::Up(button),
                PointerStage::Cancel => PointerPhase::Cancel,
            };
            let rect = Rect::from_min_size(input.pos - input.local.to_vec2(), input.size);
            let event = beui_types::input::PointerEvent {
                pointer: input.pointer,
                window: WindowId::MAIN,
                phase,
                pos: input.pos,
                modifiers: input.modifiers,
                clicks: 1,
                force: None,
                pen: None,
                predicted: None,
                time: input.time,
                source: 0,
            };
            self.handlers.on_forward.call(super::ForwardedInput {
                events: vec![beui_types::forward::Forwarded::Pointer { event, over: None }],
                rect,
                hovered: input.inside,
                focused: false,
                pointer: Some(input.pos),
                modifiers: input.modifiers,
            });
            true
        }

        fn pointer(&self, event: &web_sys::PointerEvent, stage: PointerStage) -> bool {
            if self.forward(event, stage) {
                return true;
            }
            if self.handlers.on_pointer.is_empty() {
                return false;
            }
            let Some(element) = self.element() else {
                return false;
            };
            let mut input = pointer_input(&element, event, stage);
            if stage == PointerStage::Move && event.buttons() == 0 {
                self.held.set(None);
            }
            if let Some((pointer, button)) = self.held.get()
                && pointer == event.pointer_id()
                && stage != PointerStage::Down
            {
                input.button = Some(button);
                if stage != PointerStage::Move {
                    self.held.set(None);
                }
            }
            let button = input.button;
            let taken = self.handlers.on_pointer.call(input);
            if stage == PointerStage::Down
                && taken
                && let Some(button) = button
            {
                self.held.set(Some((event.pointer_id(), button)));
                if button != PointerButton::Primary {
                    let _ = element.set_pointer_capture(event.pointer_id());
                    event.prevent_default();
                }
            }
            taken
        }

        fn ime_events(&self, events: Vec<Input>) {
            for event in events {
                let Input::Ime(event) = event else {
                    continue;
                };
                if !self.handlers.on_ime.is_empty() {
                    self.handlers.on_ime.call(event);
                    continue;
                }
                let typed = match event {
                    ImeEvent::SetComposingText(text) => {
                        *self.composed.borrow_mut() = text;
                        continue;
                    }
                    ImeEvent::CommitText(text) => {
                        self.composed.borrow_mut().clear();
                        text
                    }
                    ImeEvent::FinishComposing | ImeEvent::Disabled => {
                        std::mem::take(&mut *self.composed.borrow_mut())
                    }
                    _ => continue,
                };
                if !typed.is_empty() {
                    self.handlers.on_text.call(typed);
                }
            }
        }

        fn repeat(&self) -> Option<std::time::Duration> {
            if self.pressed.get().is_none() || !self.switches.repeat_drag.get() {
                return None;
            }
            let press = self.last_press.borrow().clone()?;
            self.handlers.on_drag.call(PointerPress {
                time: crate::frame::now(),
                ..press
            });
            reactive::flush();
            Some(REPEAT)
        }

        fn down(
            self: &Rc<Self>,
            element: &web_sys::Element,
            event: &web_sys::PointerEvent,
        ) -> bool {
            if !self.switches.press_focus.get() {
                event.prevent_default();
            }
            let mut done = self.pointer(event, PointerStage::Down);
            if event.button() == 0 {
                self.pressed.set(Some(event.pointer_id()));
                let at = press(element, event);
                *self.last_press.borrow_mut() = Some(at.clone());
                let drags = !self.handlers.on_drag.is_empty() || self.switches.grabs_pointer.get();
                if drags && !captured(event) {
                    let _ = element.set_pointer_capture(event.pointer_id());
                    let _ = js_sys::Reflect::set(event, &CAPTURED.into(), &true.into());
                }
                self.watch_release(element, event.pointer_id());
                if self.switches.pointer_locked.get() {
                    element.request_pointer_lock();
                }
                if self.switches.keyboard.get() == beui_types::SoftKeyboard::OnPress
                    && self.takes_text()
                {
                    crate::ime::show_keyboard(true);
                }
                self.handlers.on_active_change.call(true);
                self.handlers.on_press.call(at.clone());
                if self.switches.repeat_drag.get()
                    && let Some(timer) = self.repeating.get()
                {
                    timer.restart(REPEAT);
                }
                if super::covers(&self.switches.capture_presses.borrow(), at.local) {
                    done = true;
                }
            }
            done
        }

        fn watch_release(self: &Rc<Self>, element: &web_sys::Element, pointer: i32) {
            let Some(window) = crate::dom::window() else {
                return;
            };
            let mut release = self.release.borrow_mut();
            release.clear();
            for (name, stage) in [
                ("pointerup", PointerStage::Up),
                ("pointercancel", PointerStage::Cancel),
            ] {
                let held = Rc::downgrade(self);
                let element = element.clone();
                release.push(crate::dom::listen(
                    window.as_ref(),
                    name,
                    true,
                    move |event: web_sys::PointerEvent| {
                        let Some(held) = held.upgrade() else {
                            return;
                        };
                        let reaches = event
                            .target()
                            .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
                            .is_some_and(|target| element.contains(Some(&target)));
                        if event.pointer_id() == pointer && !reaches {
                            held.released(pointer, stage);
                        }
                    },
                ));
            }
        }

        fn released(&self, pointer: i32, stage: PointerStage) {
            drop(std::mem::take(&mut *self.release.borrow_mut()));
            if self.pressed.get() != Some(pointer) {
                return;
            }
            self.pressed.set(None);
            self.handlers.on_active_change.call(false);
            if stage == PointerStage::Cancel {
                self.handlers.on_cancel.call();
            }
        }

        fn claims(&self, element: &web_sys::Element, event: &web_sys::PointerEvent) -> bool {
            let claims = self.switches.claims.borrow();
            if claims.is_empty() {
                return false;
            }
            let at = placed(element, event.client_x().into(), event.client_y().into());
            let held = modifiers(event);
            claims
                .iter()
                .any(|claim| held.holds(claim.modifiers) && super::covers(&claim.area, at.local))
        }

        fn takes_text(&self) -> bool {
            self.switches.ime.get() || !self.handlers.on_text.is_empty()
        }
    }

    const POINTING: [&str; 7] = [
        "pointerdown",
        "pointermove",
        "pointerup",
        "pointercancel",
        "click",
        "auxclick",
        "contextmenu",
    ];

    fn from_overlay_within(element: &web_sys::Element, event: &web_sys::Event) -> bool {
        let mut at = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok());
        while let Some(current) = at {
            if current == *element {
                return false;
            }
            if current.matches(".beui-overlay").unwrap_or(false) {
                return true;
            }
            at = current.parent_element();
        }
        false
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
        let node = state.node;
        let on = |name: &str, handler: Box<dyn Fn(web_sys::Event)>| {
            let pointing = POINTING.contains(&name);
            let element = element.clone();
            crate::dom::listen(target, name, false, move |event: web_sys::Event| {
                if !crate::document::contains(node) {
                    return;
                }
                if pointing && from_overlay_within(&element, &event) {
                    return;
                }
                handler(event);
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
                    let refocused = event
                        .target()
                        .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
                        .is_some_and(|target| target.is_same_node(Some(&element)));
                    if refocused && held.takes_text() {
                        let sink = held.clone();
                        crate::ime::begin(
                            held.node,
                            Rc::new(move |events| sink.ime_events(events)),
                        );
                    }
                    return;
                }
                held.handlers.on_focus_change.call(true);
                if held.takes_text() {
                    let sink = held.clone();
                    crate::ime::begin(held.node, Rc::new(move |events| sink.ime_events(events)));
                    crate::ime::keyboard(held.node, held.switches.keyboard.get());
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
                let activating = !held.handlers.on_activate.is_empty()
                    || !held.handlers.on_activate_change.is_empty();
                if !done && activates && activating {
                    if !event.repeat() && !held.activating.replace(true) {
                        held.handlers.on_activate_change.call(true);
                    }
                    done = true;
                }
                let step = match event.key().as_str() {
                    "ArrowRight" | "ArrowUp" => Some(1.0),
                    "ArrowLeft" | "ArrowDown" => Some(-1.0),
                    _ => None,
                };
                if !done
                    && let Some(step) = step
                    && !held.handlers.on_step.is_empty()
                {
                    held.handlers.on_step.call(step);
                    done = true;
                }
                let types = event.key().chars().count() == 1 && !modifiers.command();
                if done && types && held.takes_text() {
                    event.stop_propagation();
                    reactive::flush();
                    return;
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
                let mut done = held.handlers.on_key.call(KeyPress {
                    key,
                    pressed: false,
                    repeat: false,
                    modifiers,
                });
                if matches!(event.key().as_str(), "Enter" | " ") && held.activating.replace(false) {
                    held.handlers.on_activate_change.call(false);
                    held.handlers.on_activate.call();
                    done = true;
                }
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
                if held.down(&element, event) {
                    event.stop_propagation();
                }
                reactive::flush();
            }),
        ));
        let held = state.clone();
        listeners.push(crate::dom::listen(
            target,
            "pointerdown",
            true,
            move |event: web_sys::Event| {
                let (Some(pointer), Some(element)) =
                    (event.dyn_ref::<web_sys::PointerEvent>(), held.element())
                else {
                    return;
                };
                if !crate::document::contains(held.node) || !held.claims(&element, pointer) {
                    return;
                }
                held.down(&element, pointer);
                event.stop_propagation();
                reactive::flush();
            },
        ));
        if !state.handlers.on_press_outside.is_empty()
            && let Some(document) = crate::dom::document()
        {
            let held = state.clone();
            listeners.push(crate::dom::listen(
                document.as_ref(),
                "pointerdown",
                true,
                move |event: web_sys::Event| {
                    let (Some(pointer), Some(element)) =
                        (event.dyn_ref::<web_sys::PointerEvent>(), held.element())
                    else {
                        return;
                    };
                    if pointer.button() != 0 || !crate::document::contains(held.node) {
                        return;
                    }
                    let inside = event
                        .target()
                        .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
                        .is_some_and(|target| element.contains(Some(&target)));
                    if !inside {
                        let at = placed(
                            &element,
                            pointer.client_x().into(),
                            pointer.client_y().into(),
                        );
                        held.handlers.on_press_outside.call(at.pos);
                        reactive::flush();
                    }
                },
            ));
        }
        let held = state.clone();
        listeners.push(on(
            "pointermove",
            Box::new(move |event| {
                let (Some(event), Some(element)) =
                    (event.dyn_ref::<web_sys::PointerEvent>(), held.element())
                else {
                    return;
                };
                let locked = crate::dom::document()
                    .and_then(|document| document.pointer_lock_element())
                    .is_some_and(|locked| locked == element);
                if locked && !held.handlers.on_motion.is_empty() {
                    held.handlers
                        .on_motion
                        .call(vec2(event.movement_x() as f32, event.movement_y() as f32));
                }
                let samples: Vec<web_sys::PointerEvent> = match held.switches.every_sample.get() {
                    true => crate::dom::call(event.as_ref(), "getCoalescedEvents", &[])
                        .map(|samples| {
                            js_sys::Array::from(&samples)
                                .iter()
                                .filter_map(|sample| sample.dyn_into().ok())
                                .collect()
                        })
                        .filter(|samples: &Vec<web_sys::PointerEvent>| !samples.is_empty())
                        .unwrap_or_else(|| vec![event.clone()]),
                    false => vec![event.clone()],
                };
                let mut done = false;
                for sample in &samples {
                    done |= held.pointer(sample, PointerStage::Move);
                    let at = press(&element, sample);
                    match held.pressed.get() {
                        Some(pointer) if pointer == event.pointer_id() => {
                            *held.last_press.borrow_mut() = Some(at.clone());
                            held.handlers.on_drag.call(at);
                        }
                        _ => held.handlers.on_hover_move.call(at),
                    }
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
                    held.released(event.pointer_id(), stage);
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
        if !state.handlers.on_wheel.is_empty() {
            listeners.push(on(
                "wheel",
                Box::new(move |event| {
                    let (Some(event), Some(element)) =
                        (event.dyn_ref::<web_sys::WheelEvent>(), held.element())
                    else {
                        return;
                    };
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
                            unclaimed: false,
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
        }
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
            held: Cell::new(None),
            pinch: RefCell::default(),
            composed: RefCell::default(),
            last_press: RefCell::default(),
            activating: Cell::new(false),
            repeating: Cell::new(None),
            release: RefCell::default(),
        });
        let held = state.clone();
        state
            .repeating
            .set(Some(crate::frame::create_timer(move || held.repeat())));
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
            let active = crate::dom::document().and_then(|document| document.active_element());
            let holds = attached.is_none()
                && active.is_some_and(|active| element.contains(Some(active.as_ref())));
            *attached = Some((element, listeners));
            drop(attached);
            if holds {
                state.handlers.on_focus_change.call(true);
                if state.takes_text() {
                    let sink = state.clone();
                    crate::ime::begin(state.node, Rc::new(move |events| sink.ime_events(events)));
                    crate::ime::keyboard(state.node, state.switches.keyboard.get());
                }
            }
            holds
        }));
    }
}

pub struct TabStop {
    node: NodeId,
    stop: Cell<bool>,
    yields: bool,
}

impl TabStop {
    pub fn new(node: NodeId, yields: bool) -> Rc<Self> {
        let stop = Rc::new(Self {
            node,
            stop: Cell::new(true),
            yields,
        });
        #[cfg(target_arch = "wasm32")]
        if yields {
            let pass = Rc::clone(&stop);
            crate::frame::on_layout(Rc::new(move || {
                pass.apply();
                false
            }));
        }
        stop
    }

    pub fn set(&self, stop: bool) {
        self.stop.set(stop);
        self.apply();
    }

    fn apply(&self) {
        let inner = self.yields && self.holds_tab_stops();
        let stop = self.stop.get() && !inner;
        crate::document::set_attribute(
            self.node,
            "tabindex",
            Some(if stop { "0" } else { "-1" }.to_owned()),
        );
    }

    #[cfg(target_arch = "wasm32")]
    fn holds_tab_stops(&self) -> bool {
        crate::dom::element(self.node)
            .and_then(|element| {
                element
                    .query_selector(
                        ":is([tabindex], a[href], button, input, select, textarea):not([tabindex=\"-1\"])",
                    )
                    .ok()
                    .flatten()
            })
            .is_some()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn holds_tab_stops(&self) -> bool {
        false
    }
}
