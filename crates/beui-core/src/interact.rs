pub mod autoscroll;
pub mod forward;
pub mod hit;
pub mod pointer;

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::base::overlay::OverlayNode;
use crate::context::Context;
use crate::geometry::Vec2;
use crate::input::{BackGesture, Event, ImeEvent, Key, KeyPress, Modifiers};
use crate::keys::{AtNode, Holder, KeyEvent, KeyInput, KeyPhase, KeyTarget, Placed, Stop};
use crate::layer::Dismiss;

use crate::current::run_in;
use crate::document::{Acted, Document};
use crate::node::{NodeId, Rects};

pub const WHEEL_LATCH_TIMEOUT: Duration = Duration::from_millis(500);
const WHEEL_LATCH_SLOP: f32 = 2.0;
pub const TOUCH_REACH: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fate {
    Taken,
    App,
    Handled { reveal: bool },
    Unheard,
}

pub fn interact(document: &mut Document, ctx: &Context, rects: &Rects, root: NodeId) {
    let events = ctx.input(|input| input.events.clone());
    let pointer_events = ctx.input(|input| input.pointer_events.clone());
    let frame = pointer::Frame::new(ctx, rects, root);
    let (long_pressed, taps) = run_in(document, |doc| {
        if events.iter().any(is_activity) {
            doc.note_input();
        }
        (pointer::begin(doc, &frame), taps(doc, &events))
    });
    if !long_pressed.is_empty() {
        run_in(document, |doc| pointer::long_pressed(doc, &frame, long_pressed));
    }
    let mut keep = Vec::with_capacity(events.len());
    let mut pointed = 0;
    for (index, event) in events.iter().enumerate() {
        while let Some(pointer_event) = pointer_events
            .get(pointed)
            .filter(|pointer_event| pointer_event.source <= index)
            .copied()
        {
            pointer_step(document, &frame, pointed, pointer_event);
            pointed += 1;
        }
        let tap = taps.contains(&index);
        let kept = match run_in(document, |doc| {
            doc.validate_focus();
            key_event(doc, ctx, event)
        }) {
            Ok(kept) => kept,
            Err(fate) => run_in(document, |doc| key_pressed(doc, event, fate, tap)),
        };
        keep.push(kept);
    }
    for (index, pointer_event) in pointer_events.iter().enumerate().skip(pointed) {
        pointer_step(document, &frame, index, *pointer_event);
    }
    run_in(document, |doc| {
        pointer::end_frame(doc, &frame);
        doc.validate_focus();
        forward::route(doc, ctx, rects, root, &events, &keep);
        if ctx.pointer_locked() && !forward::takes_keys(doc) {
            let motion = ctx.input(|input| input.pointer.motion);
            if motion != Vec2::ZERO {
                doc.motion_focused(motion);
            }
        }
        doc.validate_focus();
    });
}

fn pointer_step(
    document: &mut Document,
    frame: &pointer::Frame,
    index: usize,
    event: crate::input::PointerEvent,
) {
    if let Some(down) = run_in(document, |doc| pointer::event(doc, frame, index, event)) {
        run_in(document, |doc| pointer::finish_down(doc, frame, index, down));
    }
}

fn key_event(doc: &mut Document, ctx: &Context, event: &Event) -> Result<bool, Fate> {
    if let Some(modifiers) = event
        .modifiers()
        .filter(|_| !matches!(event, Event::PointerButton { .. }))
    {
        doc.set_modifiers(modifiers);
    }
    match event {
        Event::Focus(true) => doc.keyboard = true,
        Event::Focus(false) => {
            doc.keyboard = false;
            doc.keys.held.clear();
            doc.cancel_focus_activation();
            if ctx.pointer_locked() {
                doc.update_focus(None);
            }
        }
        Event::Back(gesture) => doc.back(*gesture),
        Event::InterceptedKey(press) => {
            offer_from_app(doc, KeyInput::Press(*press));
            return Ok(false);
        }
        Event::InterceptedTap(key) => {
            offer_from_app(doc, KeyInput::Tap(*key));
            return Ok(false);
        }
        Event::Key { press, .. } => {
            if press.pressed {
                doc.acted = Acted::Keys;
            }
            return Err(key_press(doc, *press));
        }
        Event::Text(text) if !forward::takes_keys(doc) => {
            doc.text_focused(text);
            doc.reveal_focus();
            return Ok(false);
        }
        Event::Ime(ime) if !forward::takes_keys(doc) => {
            doc.ime_focused(ime.clone());
            if !matches!(ime, ImeEvent::Enabled | ImeEvent::Disabled) {
                doc.reveal_focus();
            }
        }
        _ => {}
    }
    Ok(true)
}

fn key_pressed(doc: &mut Document, event: &Event, fate: Fate, tap: bool) -> bool {
    let Event::Key { press, text, .. } = event else {
        return true;
    };
    if let Fate::Handled { reveal: true } = fate {
        doc.reveal_focus();
    }
    if let Some(text) = text.as_ref().filter(|_| fate.local()) {
        doc.text_focused(text);
        doc.reveal_focus();
    }
    if tap && fate != Fate::Taken {
        offer_tap(doc, press.key);
    }
    fate == Fate::App
}

impl Fate {
    fn local(self) -> bool {
        matches!(self, Fate::Handled { .. } | Fate::Unheard)
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
        if Some(id) == doc.root {
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

fn offer(doc: &Document, stop: Stop, phase: KeyPhase, event: KeyEvent) -> Option<Placed> {
    let offered = KeyEvent { phase, ..event };
    doc.keys
        .at(stop, phase)
        .into_iter()
        .find(|placed| (placed.handler)(offered))
}

fn hold(doc: &mut Document, press: KeyPress, holder: Holder) {
    if press.pressed && !press.repeat {
        doc.keys.held.push((press.key, holder));
    }
}

fn key_press(doc: &mut Document, press: KeyPress) -> Fate {
    if press.pressed && !press.key.is_modifier() && autoscroll::end(doc) && press.key == Key::Escape
    {
        return Fate::Taken;
    }
    if let Some(fate) = held_key(doc, press) {
        return fate;
    }
    let route = route(doc, press.key);
    let event = KeyEvent {
        input: KeyInput::Press(press),
        phase: KeyPhase::Capture,
        target: route.target,
    };
    if route.window {
        for stop in [Stop::Host, Stop::Window] {
            if stop == Stop::Window && route.inhibited {
                break;
            }
            if let Some(placed) = offer(doc, stop, KeyPhase::Capture, event) {
                hold(doc, press, Holder::Listener(placed.handler));
                return Fate::Taken;
            }
            if stop == Stop::Window
                && press.pressed
                && press.key == Key::F6
                && forward::cycle_focus(doc, cycle_step(press.modifiers))
            {
                hold(doc, press, Holder::Cycle);
                return Fate::Taken;
            }
        }
    }
    if route.target.app {
        return Fate::App;
    }
    if press.key.is_modifier() || press.key == Key::Unidentified {
        return Fate::Unheard;
    }
    if press.pressed && !press.modifiers.ctrl && !press.modifiers.alt {
        doc.set_focus_visible(true);
    }
    let tree = Tree::gather(doc, &route);
    if let Some(placed) = tree
        .capture
        .into_iter()
        .find(|placed| (placed.handler)(event))
    {
        let reveal = placed.reveal;
        hold(doc, press, Holder::Listener(placed.handler));
        return Fate::Handled { reveal };
    }
    if let Some(node) = route.target.node
        && doc.target_key(node, press)
    {
        return Fate::Handled { reveal: true };
    }
    let bubbled = KeyEvent {
        phase: KeyPhase::Bubble,
        ..event
    };
    if let Some(placed) = tree
        .bubble
        .into_iter()
        .find(|placed| (placed.handler)(bubbled))
    {
        return Fate::Handled {
            reveal: placed.reveal,
        };
    }
    if press.pressed && press.key == Key::Escape && doc.dismiss(Dismiss::Escape) {
        return Fate::Handled { reveal: true };
    }
    if route.window
        && (offer(doc, Stop::Window, KeyPhase::Bubble, event).is_some()
            || offer(doc, Stop::Host, KeyPhase::Bubble, event).is_some())
    {
        return Fate::Handled { reveal: true };
    }
    let modifiers = press.modifiers;
    match press.key {
        Key::Tab if press.pressed && !modifiers.ctrl && !modifiers.alt => match modifiers.shift {
            true => doc.focus_previous(),
            false => doc.focus_next(),
        },
        Key::Escape if press.pressed => doc.cancel_focus_activation(),
        Key::BrowserBack if press.pressed => doc.back(BackGesture::Invoked),
        _ => return Fate::Unheard,
    }
    Fate::Handled { reveal: true }
}

fn held_key(doc: &mut Document, press: KeyPress) -> Option<Fate> {
    let at = doc
        .keys
        .held
        .iter()
        .position(|(key, _)| *key == press.key)?;
    if press.pressed && !press.repeat {
        doc.keys.held.remove(at);
        return None;
    }
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
    Some(Fate::Taken)
}

struct Tree {
    capture: Vec<Placed>,
    bubble: Vec<Placed>,
}

impl Tree {
    fn gather(doc: &Document, route: &Route) -> Self {
        let mut capture = doc.keys.placed(KeyPhase::Capture, doc.root);
        let mut bubble = doc.keys.placed(KeyPhase::Bubble, doc.root);
        let focused = route
            .target
            .node
            .is_some_and(|node| route.nodes.first() == Some(&node));
        if !focused {
            let within = scattered(doc, route);
            capture.retain(|placed| within(placed.node));
            bubble.retain(|placed| within(placed.node));
            return Self {
                capture: capture.into_iter().map(|placed| placed.placed).collect(),
                bubble: bubble.into_iter().map(|placed| placed.placed).collect(),
            };
        }
        let along = |placed: Vec<AtNode>| {
            let mut at: HashMap<NodeId, Vec<Placed>> = HashMap::new();
            for placed in placed {
                at.entry(placed.node).or_default().push(placed.placed);
            }
            move |node: &NodeId| at.remove(node).unwrap_or_default()
        };
        let mut capture_at = along(capture);
        let mut bubble_at = along(bubble);
        Self {
            capture: route.nodes.iter().rev().flat_map(&mut capture_at).collect(),
            bubble: route.nodes.iter().flat_map(&mut bubble_at).collect(),
        }
    }
}

fn scattered(doc: &Document, route: &Route) -> impl Fn(NodeId) -> bool {
    let layers = doc.layers();
    let keyboard = route
        .nodes
        .last()
        .and_then(|base| doc.layer_of(&layers, *base));
    move |node| {
        doc.contains(node)
            && doc.live(node)
            && keyboard.is_some_and(|keyboard| {
                doc.layer_of(&layers, node)
                    .is_some_and(|layer| layers.within(layer, keyboard))
            })
    }
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
}

fn is_activity(event: &Event) -> bool {
    matches!(
        event,
        Event::Key { .. }
            | Event::PointerButton { .. }
            | Event::PointerMotion(_)
            | Event::PointerMoved(_)
            | Event::Scroll(_)
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

fn taps(doc: &mut Document, events: &[Event]) -> Vec<usize> {
    let mut taps = Vec::new();
    for (index, event) in events.iter().enumerate() {
        match *event {
            Event::Key { press, .. } if press.pressed && !press.repeat => {
                doc.tapping = (press.key.is_modifier() && alone(press.key, press.modifiers))
                    .then_some(press.key);
            }
            Event::Key { press, .. } if !press.pressed && doc.tapping == Some(press.key) => {
                doc.tapping = None;
                taps.push(index);
            }
            Event::Key { press, .. } if press.pressed => doc.tapping = None,
            Event::PointerButton { pressed: true, .. }
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
