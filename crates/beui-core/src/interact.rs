pub mod autoscroll;
pub mod forward;
pub mod hit;
pub mod pointer;

use foldhash::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::base::overlay::OverlayNode;
use crate::context::Context;
use crate::geometry::Vec2;
use crate::input::{BackGesture, Event, ImeEvent, Key, KeyPress, Modifiers};
use crate::keys::{AtNode, KeyEvent, KeyHandler, KeyInput, KeyPhase, KeyTarget, Placed, Stop};
use crate::layer::Dismiss;

use crate::current::{run_in, with_document};
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
    let frame = pointer::Frame::new(ctx, rects);
    let (long_pressed, taps) = run_in(document, |doc| {
        if events.iter().any(is_activity) {
            doc.note_input();
        }
        (pointer::begin(doc, &frame), taps(doc, &events))
    });
    if !long_pressed.is_empty() {
        run_in(document, |doc| {
            pointer::long_pressed(doc, &frame, long_pressed)
        });
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
        if let Some(internals) = ctx.comps_query() {
            ctx.answer_comps(doc.component_tree().text(doc, internals));
        }
        if let Some(pos) = ctx.hit_query()
            && rects.visible(&root).is_some_and(|rect| rect.contains(pos))
        {
            ctx.answer_hit(hit::describe(doc, rects, pos, ctx.pixels_per_point()));
        }
        doc.validate_focus();
        forward::route(doc, ctx, rects, &events, &keep);
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
        run_in(document, |doc| {
            pointer::finish_down(doc, frame, index, down)
        });
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
        Event::Focus(true) => doc.set_keyboard(true),
        Event::Focus(false) => {
            doc.set_keyboard(false);
            doc.pointers.space = false;
            doc.keys.held.clear();
            doc.cancel_focus_activation();
            if ctx.pointer_locked() {
                doc.update_focus(None);
            }
        }
        Event::PointerUnlocked if ctx.pointer_locked() => doc.update_focus(None),
        Event::Back(gesture) => doc.back(*gesture),
        Event::InterceptedKey(press) => {
            from_app(doc, KeyInput::Press(*press));
            return Ok(false);
        }
        Event::InterceptedTap(key) => {
            from_app(doc, KeyInput::Tap(*key));
            return Ok(false);
        }
        Event::Key { press, .. } => {
            if press.key == Key::Space && !press.repeat {
                doc.pointers.space = press.pressed;
            }
            if press.pressed {
                doc.acted = Acted::Keys;
                if !press.key.is_media() {
                    doc.touched = false;
                    doc.keyboard_held = false;
                }
            }
            let asked = doc.paste_requested();
            let fate = key_press(doc, *press);
            let asks = matches!(fate, Fate::App | Fate::Unheard) || doc.paste_requested() && !asked;
            if asks && pastes(*press) {
                ctx.paste_shortcut();
            }
            return Err(fate);
        }
        Event::Text(text) if !forward::takes_keys(doc) => {
            doc.text_focused(text);
            doc.reveal_focus();
            return Ok(false);
        }
        Event::Paste(pasted) if !forward::takes_keys(doc) => {
            doc.paste_focused(pasted.clone());
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

fn pastes(press: KeyPress) -> bool {
    press.pressed && press.key == Key::V && press.modifiers.ctrl && !press.modifiers.alt
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
        let route = route(doc, press.key);
        if !route.inhibited {
            let event = event_for(KeyInput::Tap(press.key), &route);
            let _ = answer(outer(doc, &route, KeyPhase::Capture), event);
        }
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
        window: !layers.locked() || key.is_media(),
        inhibited,
    }
}

fn event_for(input: KeyInput, route: &Route) -> KeyEvent {
    KeyEvent {
        input,
        phase: KeyPhase::Capture,
        target: route.target,
    }
}

fn answer(placed: Vec<Placed>, event: KeyEvent) -> Option<Placed> {
    placed.into_iter().find(|placed| (placed.handler)(event))
}

fn outer(doc: &Document, route: &Route, phase: KeyPhase) -> Vec<Placed> {
    let own = |handler: fn(KeyEvent) -> bool| Placed {
        handler: Rc::new(handler),
        reveal: true,
    };
    let listening = |stop: Stop| match route.window && !(stop == Stop::Window && route.inhibited) {
        true => doc.keys.at(stop, phase, doc.key_window()),
        false => Vec::new(),
    };
    let mut placed = Vec::new();
    match phase {
        KeyPhase::Capture => {
            placed.push(own(builtin::autoscroll));
            placed.extend(listening(Stop::Host));
            placed.extend(listening(Stop::Window));
            if route.window && !route.inhibited {
                placed.push(own(builtin::cycle));
            }
        }
        _ => {
            placed.push(own(builtin::dismiss));
            placed.extend(listening(Stop::Window));
            placed.extend(listening(Stop::Host));
            placed.push(own(builtin::defaults));
            for placed in &mut placed {
                placed.reveal = true;
            }
        }
    }
    placed
}

mod builtin {
    use super::{
        BackGesture, Dismiss, Key, KeyEvent, KeyPhase, autoscroll, cycle_step, forward,
        with_document,
    };

    pub(super) fn autoscroll(event: KeyEvent) -> bool {
        event.phase != KeyPhase::Held
            && event.press().is_some_and(|press| {
                press.pressed
                    && !press.key.is_modifier()
                    && with_document(autoscroll::end)
                    && press.key == Key::Escape
            })
    }

    pub(super) fn cycle(event: KeyEvent) -> bool {
        let Some(press) = event.press().filter(|press| press.pressed) else {
            return event.held();
        };
        let step = cycle_step(press.modifiers);
        match event.held() {
            true => with_document(|doc| {
                if !(forward::takes_keys(doc) && forward::shortcuts_inhibited(doc)) {
                    forward::cycle_focus(doc, step);
                }
                true
            }),
            false => press.key == Key::F6 && with_document(|doc| forward::cycle_focus(doc, step)),
        }
    }

    pub(super) fn dismiss(event: KeyEvent) -> bool {
        event.press().is_some_and(|press| {
            press.pressed
                && press.key == Key::Escape
                && with_document(|doc| doc.dismiss(Dismiss::Escape))
        })
    }

    pub(super) fn defaults(event: KeyEvent) -> bool {
        let Some(press) = event.press().filter(|press| press.pressed) else {
            return false;
        };
        let modifiers = press.modifiers;
        with_document(|doc| match press.key {
            Key::Tab if !modifiers.ctrl && !modifiers.alt => {
                match modifiers.shift {
                    true => doc.focus_previous(),
                    false => doc.focus_next(),
                }
                true
            }
            Key::Escape => {
                doc.cancel_focus_activation();
                true
            }
            Key::BrowserBack => {
                doc.back(BackGesture::Invoked);
                true
            }
            _ => false,
        })
    }
}

fn hold(doc: &mut Document, press: KeyPress, handler: Rc<KeyHandler>, repeats: bool) {
    if press.pressed && !press.repeat {
        doc.keys.held.push((press.key, handler, repeats));
    }
}

fn target_holder(node: NodeId) -> Rc<KeyHandler> {
    Rc::new(move |event: KeyEvent| {
        event
            .press()
            .is_some_and(|press| with_document(|doc| doc.target_key(node, press)))
    })
}

fn key_press(doc: &mut Document, press: KeyPress) -> Fate {
    if let Some(fate) = held_key(doc, press) {
        return fate;
    }
    let route = route(doc, press.key);
    let event = event_for(KeyInput::Press(press), &route);
    if let Some(placed) = answer(outer(doc, &route, KeyPhase::Capture), event) {
        hold(doc, press, placed.handler, true);
        return Fate::Taken;
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
    if let Some(placed) = answer(tree.capture, event) {
        let reveal = placed.reveal;
        hold(doc, press, placed.handler, true);
        return Fate::Handled { reveal };
    }
    if let Some(node) = route.target.node
        && doc.target_key(node, press)
    {
        hold(doc, press, target_holder(node), false);
        return Fate::Handled { reveal: true };
    }
    let bubbled = KeyEvent {
        phase: KeyPhase::Bubble,
        ..event
    };
    let mut bubble = tree.bubble;
    bubble.extend(outer(doc, &route, KeyPhase::Bubble));
    match answer(bubble, bubbled) {
        Some(placed) => {
            let reveal = placed.reveal;
            hold(doc, press, placed.handler, false);
            Fate::Handled { reveal }
        }
        None => Fate::Unheard,
    }
}

fn held_key(doc: &mut Document, press: KeyPress) -> Option<Fate> {
    let at = doc
        .keys
        .held
        .iter()
        .position(|(key, _, _)| *key == press.key)?;
    if press.pressed && !press.repeat {
        doc.keys.held.remove(at);
        return None;
    }
    if press.pressed && !doc.keys.held[at].2 {
        return None;
    }
    let route = route(doc, press.key);
    let handler = match press.pressed {
        true => Rc::clone(&doc.keys.held[at].1),
        false => doc.keys.held.remove(at).1,
    };
    handler(KeyEvent {
        input: KeyInput::Press(press),
        phase: KeyPhase::Held,
        target: route.target,
    });
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
            let mut at: HashMap<NodeId, Vec<Placed>> = HashMap::default();
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

fn from_app(doc: &mut Document, input: KeyInput) {
    let media = matches!(input, KeyInput::Press(press) if press.key.is_media());
    let route = Route {
        target: KeyTarget {
            node: None,
            typing: true,
            app: true,
        },
        nodes: Vec::new(),
        window: !doc.layers().locked() || media,
        inhibited: false,
    };
    let event = event_for(input, &route);
    let _ = answer(outer(doc, &route, KeyPhase::Capture), event);
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
            | Event::Paste(_)
            | Event::Touch { .. }
            | Event::Pen { .. }
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
            }
            | Event::Pen {
                phase: crate::input::PenPhase::Down(_),
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
