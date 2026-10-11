pub mod autoscroll;
pub mod forward;
pub mod hit;
pub mod pointer;

use std::time::Duration;

use crate::base::overlay::OverlayNode;
use crate::context::Context;
use crate::geometry::Vec2;
use crate::input::{BackGesture, Event, ImeEvent, Key, KeyPress, Modifiers};
use crate::keys::{Holder, KeyEvent, KeyHandler, KeyInput, KeyPhase, KeyTarget, Stop};
use crate::layer::Dismiss;
use crate::painter::Painter;

use crate::document::{Acted, Document};
use crate::node::{NodeId, Rects};

pub const WHEEL_LATCH_TIMEOUT: Duration = Duration::from_millis(500);
const WHEEL_LATCH_SLOP: f32 = 2.0;
pub const TOUCH_REACH: f32 = 12.0;

#[derive(Clone, Copy)]
pub enum Keys {
    Ignored,
    All,
    Except(fn(&Event) -> bool),
}

impl Keys {
    pub fn ignored(self) -> bool {
        matches!(self, Self::Ignored)
    }

    fn claimed(self, event: &Event) -> bool {
        match self {
            Self::Except(claimed) => claimed(event),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fate {
    Taken,
    App,
    Handled { reveal: bool },
    Unheard,
}

pub fn interact(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    rects: &Rects,
    root: NodeId,
    pointer: bool,
    keys: Keys,
) {
    let events = ctx.input(|input| input.events.clone());
    if events.iter().any(is_activity) {
        doc.note_input();
    }
    let typed = !keys.ignored()
        && events
            .iter()
            .any(|event| matches!(event, Event::Key { pressed: true, .. }));
    let moved = events
        .iter()
        .any(|event| matches!(event, Event::PointerMoved(_)));
    if typed {
        doc.acted = Acted::Keys;
    } else if pointer
        && moved
        && let Some(pos) = ctx.input(|input| input.pointer.interact_pos())
    {
        doc.acted = Acted::Pointer(pos + doc.pointers.shift);
    }
    let dispatched = pointer::dispatch(doc, ctx, rects, root, pointer);
    doc.validate_focus();
    let keep = match keys.ignored() {
        true => vec![true; events.len()],
        false => key_events(doc, ctx, painter, &events, keys, dispatched.swallows_escape),
    };
    forward::route(doc, ctx, rects, root, &events, &keep, pointer, keys);
    if ctx.pointer_locked() && pointer && !keys.ignored() && !forward::takes_keys(doc) {
        let motion = ctx.input(|input| input.pointer.motion);
        if motion != Vec2::ZERO {
            doc.motion_focused(motion);
        }
    }
    doc.validate_focus();
}

fn key_events(
    doc: &mut Document,
    ctx: &Context,
    painter: &Painter,
    events: &[Event],
    keys: Keys,
    swallows_escape: bool,
) -> Vec<bool> {
    let mut keep = vec![true; events.len()];
    let mut taken = vec![false; events.len()];
    let taps = taps(doc, events);
    for (index, event) in events.iter().enumerate() {
        ::reactive::settle(|| {});
        doc.validate_focus();
        if let Event::Modifiers(modifiers) | Event::Key { modifiers, .. } = event {
            doc.set_modifiers(*modifiers);
        }
        match event {
            Event::Focus(false) => {
                doc.keys.held.clear();
                doc.cancel_focus_activation();
                if ctx.pointer_locked() {
                    doc.update_focus(None);
                }
            }
            Event::Back(gesture) => doc.back(*gesture),
            Event::InterceptedKey(press) => {
                doc.set_modifiers(press.modifiers);
                offer_from_app(doc, KeyInput::Press(*press));
                taken[index] = true;
            }
            Event::InterceptedTap(key) => {
                offer_from_app(doc, KeyInput::Tap(*key));
                taken[index] = true;
            }
            Event::Key {
                key: Key::Escape, ..
            } if swallows_escape => taken[index] = true,
            Event::Key { .. } if keys.claimed(event) => {}
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => {
                let press = KeyPress {
                    key: *key,
                    pressed: *pressed,
                    repeat: *repeat,
                    modifiers: *modifiers,
                };
                let fate = key_press(doc, press);
                match fate {
                    Fate::Taken => drop_key(&mut taken, events, index, press.pressed),
                    Fate::App => {}
                    Fate::Handled { reveal: true } => {
                        doc.reveal_focus(painter);
                        keep_off(&mut keep, events, index);
                    }
                    Fate::Handled { reveal: false } | Fate::Unheard => {
                        keep_off(&mut keep, events, index);
                    }
                }
                ::reactive::settle(|| {});
                if taps.contains(&index) && fate != Fate::Taken {
                    offer_tap(doc, *key);
                }
            }
            Event::Text(text) if !taken[index] => match forward::takes_keys(doc) {
                true => {}
                false => {
                    keep[index] = false;
                    doc.text_focused(text);
                    doc.reveal_focus(painter);
                }
            },
            Event::Ime(ime) if !taken[index] => match forward::takes_keys(doc) {
                true => {}
                false => {
                    let reveals = !matches!(ime, ImeEvent::Enabled | ImeEvent::Disabled);
                    doc.ime_focused(ime.clone());
                    if reveals {
                        doc.reveal_focus(painter);
                    }
                }
            },
            _ => {}
        }
    }
    if taken.iter().any(|taken| *taken) {
        let mut taken = taken.iter();
        ctx.retain_events(|_| !taken.next().copied().unwrap_or(false));
    }
    for (keep, taken) in keep.iter_mut().zip(taken) {
        *keep &= !taken;
    }
    keep
}

fn keep_off(keep: &mut [bool], events: &[Event], index: usize) {
    keep[index] = false;
    for before in (0..index).rev() {
        match events[before] {
            Event::Modifiers(_) | Event::Text(_) => {}
            Event::PhysicalKey { .. } => {
                keep[before] = false;
                break;
            }
            _ => break,
        }
    }
}

struct Route {
    target: KeyTarget,
    nodes: Vec<NodeId>,
    window: bool,
    inhibited: bool,
}

fn route(doc: &Document, key: Key) -> Route {
    let layers = doc.layers();
    let focused = doc.focused.filter(|focused| doc.contains(*focused));
    let app = forward::takes_keys(doc);
    let inhibited = app && forward::shortcuts_inhibited(doc);
    let typing = app || doc.focus_types();
    let start = focused.or_else(|| layers[layers.keyboard()].content);
    let mut nodes = Vec::new();
    let mut current = start;
    while let Some(id) = current {
        if let Some(overlay) = doc.arena.kind_of::<OverlayNode>(id)
            && layers
                .index_of(overlay)
                .is_some_and(|index| layers[index].traps)
        {
            break;
        }
        nodes.push(id);
        if Some(id) == doc.root || nodes.len() > 4096 {
            break;
        }
        current = doc.arena.parent(id);
    }
    Route {
        target: KeyTarget {
            node: focused,
            typing,
            app,
        },
        nodes,
        window: !layers.locks_window() || key.is_media(),
        inhibited,
    }
}

fn offer(doc: &Document, stop: Stop, phase: KeyPhase, event: KeyEvent) -> Option<Rc<KeyHandler>> {
    let offered = KeyEvent { phase, ..event };
    doc.keys
        .at(stop, phase, doc.root)
        .into_iter()
        .find(|handler| handler(offered))
}

use std::rc::Rc;

fn key_press(doc: &mut Document, press: KeyPress) -> Fate {
    if let Some(at) = doc.keys.held.iter().position(|(key, _)| *key == press.key) {
        if press.pressed && !press.repeat {
            doc.keys.held.remove(at);
        } else {
            let route = route(doc, press.key);
            let holder = match press.pressed {
                true => match &doc.keys.held[at].1 {
                    Holder::Listener(handler) => Some(Rc::clone(handler)),
                    Holder::Cycle => None,
                },
                false => match doc.keys.held.remove(at).1 {
                    Holder::Listener(handler) => Some(handler),
                    Holder::Cycle => None,
                },
            };
            match holder {
                Some(handler) => {
                    handler(KeyEvent {
                        input: KeyInput::Press(press),
                        phase: KeyPhase::Held,
                        target: route.target,
                    });
                }
                None if press.pressed && !route.inhibited => {
                    forward::cycle_focus(doc, cycle_step(press.modifiers));
                }
                None => {}
            }
            return Fate::Taken;
        }
    }
    let route = route(doc, press.key);
    let event = KeyEvent {
        input: KeyInput::Press(press),
        phase: KeyPhase::Capture,
        target: route.target,
    };
    let holds = press.pressed && !press.repeat;
    let mut stops = Vec::new();
    if route.window {
        stops.push(Stop::Host);
        if !route.inhibited {
            stops.push(Stop::Window);
        }
    }
    for stop in stops {
        if let Some(handler) = offer(doc, stop, KeyPhase::Capture, event) {
            if holds {
                doc.keys.held.push((press.key, Holder::Listener(handler)));
            }
            return Fate::Taken;
        }
        if stop == Stop::Window
            && press.pressed
            && press.key == Key::F6
            && forward::cycle_focus(doc, cycle_step(press.modifiers))
        {
            if holds {
                doc.keys.held.push((press.key, Holder::Cycle));
            }
            return Fate::Taken;
        }
    }
    if route.target.app {
        return Fate::App;
    }
    if press.key.is_modifier() {
        return Fate::Unheard;
    }
    if press.pressed && !press.modifiers.ctrl && !press.modifiers.alt {
        doc.set_focus_visible(true);
    }
    let target = route
        .target
        .node
        .filter(|node| route.nodes.first() == Some(node));
    let ancestors = match target {
        Some(_) => &route.nodes[1..],
        None => &route.nodes[..],
    };
    let unfocused = match target {
        Some(_) => None,
        None => Some(scattered(doc, &route)),
    };
    match &unfocused {
        Some((capture, _)) => {
            if let Some(handler) = capture.iter().find(|handler| handler(event)) {
                if holds {
                    doc.keys
                        .held
                        .push((press.key, Holder::Listener(Rc::clone(handler))));
                }
                return Fate::Handled { reveal: true };
            }
        }
        None => {
            for node in ancestors.iter().rev() {
                if let Some(handler) = offer(doc, Stop::Node(*node), KeyPhase::Capture, event) {
                    if holds {
                        doc.keys.held.push((press.key, Holder::Listener(handler)));
                    }
                    return Fate::Handled { reveal: true };
                }
            }
        }
    }
    if let Some(node) = target
        && target_key(doc, node, press)
    {
        return Fate::Handled { reveal: true };
    }
    match &unfocused {
        Some((_, bubble)) => {
            let bubbled = KeyEvent {
                phase: KeyPhase::Bubble,
                ..event
            };
            if bubble.iter().any(|handler| handler(bubbled)) {
                return Fate::Handled { reveal: true };
            }
        }
        None => {
            for node in ancestors {
                if ancestor_key(doc, *node, press) {
                    return Fate::Handled { reveal: false };
                }
                if offer(doc, Stop::Node(*node), KeyPhase::Bubble, event).is_some() {
                    return Fate::Handled { reveal: true };
                }
            }
        }
    }
    if route.window
        && (offer(doc, Stop::Window, KeyPhase::Bubble, event).is_some()
            || offer(doc, Stop::Host, KeyPhase::Bubble, event).is_some())
    {
        return Fate::Handled { reveal: true };
    }
    let modifiers = press.modifiers;
    match press.key {
        Key::Tab if press.pressed && !modifiers.ctrl && !modifiers.alt => {
            if modifiers.shift {
                doc.focus_previous();
            } else {
                doc.focus_next();
            }
        }
        Key::Escape if press.pressed => {
            if !doc.dismiss(Dismiss::Escape) {
                doc.cancel_focus_activation();
            }
        }
        Key::BrowserBack if press.pressed => doc.back(BackGesture::Invoked),
        _ => return Fate::Unheard,
    }
    Fate::Handled { reveal: true }
}

fn scattered(doc: &Document, route: &Route) -> (Vec<Rc<KeyHandler>>, Vec<Rc<KeyHandler>>) {
    let Some(base) = route.nodes.last().copied() else {
        return (Vec::new(), Vec::new());
    };
    let layers = doc.layers();
    let keyboard = doc.layer_of(&layers, base);
    let within = |node: NodeId| {
        doc.contains(node)
            && doc.live(node)
            && keyboard.is_some_and(|keyboard| {
                doc.layer_of(&layers, node)
                    .is_some_and(|layer| layers.within(layer, keyboard))
            })
    };
    let gather = |phase: KeyPhase| {
        doc.keys
            .placed(phase, doc.root)
            .into_iter()
            .filter(|(node, _)| within(*node))
            .map(|(_, handler)| handler)
            .collect()
    };
    (gather(KeyPhase::Capture), gather(KeyPhase::Bubble))
}

fn target_key(doc: &mut Document, node: NodeId, press: KeyPress) -> bool {
    if doc.key_focused(press) {
        return true;
    }
    let modifiers = press.modifiers;
    match press.key {
        Key::Enter | Key::Space if !press.pressed => {
            if doc.activation_key == Some(press.key) {
                doc.set_focus_key_pressed(press.key, false, press.repeat);
                return true;
            }
            false
        }
        Key::Enter | Key::Space if !modifiers.ctrl && !modifiers.alt => {
            if !doc.activates(node) {
                return false;
            }
            doc.set_focus_key_pressed(press.key, true, press.repeat);
            true
        }
        Key::ArrowLeft | Key::ArrowDown | Key::ArrowRight | Key::ArrowUp
            if press.pressed && !modifiers.ctrl && !modifiers.alt && doc.steps(node) =>
        {
            let delta = match press.key {
                Key::ArrowLeft | Key::ArrowDown => -1.0,
                _ => 1.0,
            };
            doc.step_focused(delta);
            true
        }
        _ => false,
    }
}

fn ancestor_key(doc: &mut Document, node: NodeId, press: KeyPress) -> bool {
    let handler = hit::interactive(doc, node)
        .and_then(|node| node.focus.as_ref())
        .map(|focus| focus.on_ancestor_key.clone());
    handler.is_some_and(|handler| !handler.is_empty() && handler.call(press))
}

fn offer_from_app(doc: &mut Document, input: KeyInput) {
    let layers = doc.layers();
    if layers.locks_window() && !matches!(input, KeyInput::Press(press) if press.key.is_media()) {
        return;
    }
    let event = KeyEvent {
        input,
        phase: KeyPhase::Capture,
        target: KeyTarget {
            node: None,
            typing: true,
            app: true,
        },
    };
    let _ = offer(doc, Stop::Host, KeyPhase::Capture, event).is_some()
        || offer(doc, Stop::Window, KeyPhase::Capture, event).is_some();
    ::reactive::settle(|| {});
}

fn offer_tap(doc: &mut Document, key: Key) {
    let route = route(doc, key);
    if !route.window || route.inhibited {
        return;
    }
    let event = KeyEvent {
        input: KeyInput::Tap(key),
        phase: KeyPhase::Capture,
        target: route.target,
    };
    let _ = offer(doc, Stop::Host, KeyPhase::Capture, event).is_some()
        || offer(doc, Stop::Window, KeyPhase::Capture, event).is_some();
    ::reactive::settle(|| {});
}

fn is_activity(event: &Event) -> bool {
    matches!(
        event,
        Event::Key { .. }
            | Event::PointerButton { .. }
            | Event::PointerMotion(_)
            | Event::PointerMoved(_)
            | Event::Scroll(_)
            | Event::PhysicalKey { .. }
            | Event::Text(_)
            | Event::Touch { .. }
            | Event::Zoom(_)
    )
}

fn cycle_step(modifiers: Modifiers) -> isize {
    match modifiers.shift {
        true => -1,
        false => 1,
    }
}

fn drop_key(taken: &mut [bool], events: &[Event], index: usize, pressed: bool) {
    taken[index] = true;
    for before in (0..index).rev() {
        match events[before] {
            Event::Modifiers(_) | Event::Text(_) => {}
            Event::PhysicalKey {
                pressed: physical, ..
            } if physical == pressed => {
                taken[before] = true;
                break;
            }
            _ => break,
        }
    }
    for after in index + 1..events.len() {
        match events[after] {
            Event::Text(_) => taken[after] = true,
            _ => break,
        }
    }
}

fn taps(doc: &mut Document, events: &[Event]) -> Vec<usize> {
    let mut taps = Vec::new();
    for (index, event) in events.iter().enumerate() {
        match *event {
            Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
            } => doc.tapping = (key.is_modifier() && alone(key, modifiers)).then_some(key),
            Event::Key {
                key,
                pressed: false,
                ..
            } if doc.tapping == Some(key) => {
                doc.tapping = None;
                taps.push(index);
            }
            Event::Key { pressed: true, .. }
            | Event::PointerButton { pressed: true, .. }
            | Event::Scroll(_)
            | Event::Focus(false)
            | Event::Touch {
                phase: crate::input::TouchPhase::Start,
                ..
            } => doc.tapping = None,
            _ => {}
        }
    }
    taps
}

fn alone(key: Key, held: Modifiers) -> bool {
    Modifiers {
        alt: held.alt && key != Key::Alt,
        ctrl: held.ctrl && key != Key::Ctrl,
        shift: held.shift && key != Key::Shift,
        logo: held.logo && key != Key::Logo,
    } == Modifiers::NONE
}
