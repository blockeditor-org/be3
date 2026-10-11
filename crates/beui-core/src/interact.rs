pub mod autoscroll;
pub mod forward;
pub mod hit;
pub mod pointer;

use std::time::Duration;

use crate::context::Context;
use crate::geometry::Vec2;
use crate::input::{BackGesture, Event, ImeEvent, Key, KeyPress, Modifiers};
use crate::painter::Painter;

use crate::document::{Acted, Document, GlobalKeyPress};
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
        false => global_keys(doc, ctx, &events),
    };
    forward::route(doc, ctx, rects, root, &events, &keep, pointer, keys);
    let forwarded_keys = forward::takes_keys(doc);
    if ctx.pointer_locked() && pointer && !keys.ignored() && !forwarded_keys {
        let motion = ctx.input(|input| input.pointer.motion);
        if motion != Vec2::ZERO {
            doc.motion_focused(motion);
        }
        doc.validate_focus();
    }
    let swallows_escape = dispatched.swallows_escape;

    for event in ctx.input(|input| input.events.clone()) {
        ::reactive::settle(|| {});
        doc.validate_focus();
        if let Event::Modifiers(modifiers) | Event::Key { modifiers, .. } = &event {
            doc.set_modifiers(*modifiers);
        }
        let (key, pressed, repeat, modifiers) = match event {
            Event::Key {
                key: Key::Escape, ..
            } if swallows_escape => continue,
            Event::Focus(false) => {
                doc.cancel_focus_activation();
                if ctx.pointer_locked() {
                    doc.update_focus(None);
                }
                continue;
            }
            Event::Back(gesture) if !keys.ignored() => {
                doc.back(gesture);
                continue;
            }
            Event::Text(_) | Event::Key { .. } | Event::Ime(_) if keys.ignored() => continue,
            Event::Text(_) | Event::Key { .. } | Event::Ime(_) if forwarded_keys => continue,
            Event::Key { key, .. } if key.is_modifier() || keys.claimed(&event) => {
                continue;
            }
            Event::Text(text) => {
                doc.text_focused(&text);
                doc.reveal_focus(painter);
                continue;
            }
            Event::Ime(ime) => {
                let reveals = !matches!(ime, ImeEvent::Enabled | ImeEvent::Disabled);
                doc.ime_focused(ime);
                if reveals {
                    doc.reveal_focus(painter);
                }
                continue;
            }
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => (key, pressed, repeat, modifiers),
            _ => continue,
        };
        let press = KeyPress {
            key,
            pressed,
            repeat,
            modifiers,
        };
        if pressed && !modifiers.ctrl && !modifiers.alt {
            doc.set_focus_visible(true);
        }
        if doc.overlay_stack.is_empty() && doc.key_shortcut(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.key_focused(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.overlay_stack.is_empty() && doc.key_unhandled(press) {
            doc.reveal_focus(painter);
            continue;
        }
        if doc.key_ancestor(press) {
            continue;
        }
        match key {
            Key::Tab if pressed && !modifiers.ctrl && !modifiers.alt => {
                if modifiers.shift {
                    doc.focus_previous();
                } else {
                    doc.focus_next();
                }
            }
            Key::Escape if pressed && !doc.overlay_stack.is_empty() => {
                doc.dismiss_topmost_overlay();
            }
            Key::Escape if pressed => doc.cancel_focus_activation(),
            Key::BrowserBack if pressed => doc.back(BackGesture::Invoked),
            Key::Enter | Key::Space if !pressed || (!modifiers.ctrl && !modifiers.alt) => {
                doc.set_focus_key_pressed(key, pressed, repeat);
            }
            Key::ArrowLeft | Key::ArrowDown if pressed && !modifiers.ctrl && !modifiers.alt => {
                doc.step_focused(-1.0)
            }
            Key::ArrowRight | Key::ArrowUp if pressed && !modifiers.ctrl && !modifiers.alt => {
                doc.step_focused(1.0)
            }
            _ => {}
        }
        doc.reveal_focus(painter);
    }
    doc.validate_focus();
}

fn global_keys(doc: &mut Document, ctx: &Context, events: &[Event]) -> Vec<bool> {
    let mut keep = vec![true; events.len()];
    let taps = taps(doc, events);
    if events
        .iter()
        .any(|event| matches!(event, Event::Focus(false)))
    {
        doc.globally_held.clear();
    }
    if !events.iter().any(|event| {
        matches!(
            event,
            Event::Key { .. } | Event::InterceptedKey(_) | Event::InterceptedTap(_)
        )
    }) {
        return keep;
    }
    for (index, event) in events.iter().enumerate() {
        let (key, pressed, repeat, modifiers) = match *event {
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => (key, pressed, repeat, modifiers),
            Event::Focus(false) => {
                doc.globally_held.clear();
                continue;
            }
            Event::InterceptedKey(press) => {
                doc.set_modifiers(press.modifiers);
                doc.key_global(GlobalKeyPress {
                    press,
                    typing: true,
                    in_app: true,
                    inhibited: false,
                    held: false,
                    tap: false,
                });
                ::reactive::settle(|| {});
                keep[index] = false;
                continue;
            }
            Event::InterceptedTap(key) => {
                doc.key_global(GlobalKeyPress {
                    press: KeyPress {
                        key,
                        pressed: false,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    },
                    typing: true,
                    in_app: true,
                    inhibited: false,
                    held: false,
                    tap: true,
                });
                ::reactive::settle(|| {});
                keep[index] = false;
                continue;
            }
            _ => continue,
        };
        doc.set_modifiers(modifiers);
        let in_app = forward::takes_keys(doc);
        let inhibited = in_app && forward::shortcuts_inhibited(doc);
        let typing = in_app || doc.focus_types();
        let press = KeyPress {
            key,
            pressed,
            repeat,
            modifiers,
        };
        let offered = GlobalKeyPress {
            press,
            typing,
            in_app,
            inhibited,
            held: false,
            tap: false,
        };
        if pressed && !repeat {
            doc.globally_held.retain(|(other, _)| *other != key);
        }
        let taken = match doc
            .globally_held
            .iter()
            .position(|(other, _)| *other == key)
        {
            Some(at) => {
                let taker = doc.globally_held[at].1.clone();
                if !pressed {
                    doc.globally_held.remove(at);
                }
                match taker {
                    Some(taker) => {
                        taker(GlobalKeyPress {
                            held: true,
                            ..offered
                        });
                    }
                    None if pressed && !inhibited => {
                        forward::cycle_focus(doc, cycle_step(modifiers));
                    }
                    None => {}
                }
                true
            }
            None => {
                let taker = match doc.global_taker(offered) {
                    Some(taker) => Some(Some(taker)),
                    None if pressed && key == Key::F6 && !inhibited => {
                        forward::cycle_focus(doc, cycle_step(modifiers)).then_some(None)
                    }
                    None => None,
                };
                if pressed && let Some(taker) = &taker {
                    doc.globally_held.push((key, taker.clone()));
                }
                taker.is_some()
            }
        };
        ::reactive::settle(|| {});
        if taken {
            drop_key(&mut keep, events, index, pressed);
        }
        if taps.contains(&index) && !inhibited {
            doc.key_global(GlobalKeyPress {
                tap: true,
                ..offered
            });
            ::reactive::settle(|| {});
        }
    }
    if keep.iter().all(|keep| *keep) {
        return keep;
    }
    let mut kept = keep.clone().into_iter();
    ctx.retain_events(|_| kept.next().unwrap_or(true));
    keep
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

fn drop_key(keep: &mut [bool], events: &[Event], index: usize, pressed: bool) {
    keep[index] = false;
    for before in (0..index).rev() {
        match events[before] {
            Event::Modifiers(_) | Event::Text(_) => {}
            Event::PhysicalKey {
                pressed: physical, ..
            } if physical == pressed => {
                keep[before] = false;
                break;
            }
            _ => break,
        }
    }
    for after in index + 1..events.len() {
        match events[after] {
            Event::Text(_) => keep[after] = false,
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
