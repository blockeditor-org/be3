use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use be_lib_geometry::{Pos2, Rect};

use crate::input::{
    Event, Input, KeyPress, Keystroke, Modifiers, PenPhase, PointerEvent, PointerId, PointerPhase,
    ScrollDelta, TouchPhase,
};
use crate::node::NodeId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    pub node: NodeId,
    pub local: Pos2,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Forwarded {
    Pointer {
        event: PointerEvent,
        over: Option<Over>,
    },
    Left,
    Focus(bool),
    /// A key pressed while this catcher had the focus that something else took: an action that
    /// captures it, such as the desktop's Alt+Tab. The catcher heard the keys held before it, so a
    /// program would take their release as a lone tap (Alt alone opens Firefox's menu bar).
    KeyTaken,
    Input(Event),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForwardedInput {
    pub events: Vec<Forwarded>,
    pub rect: Rect,
    pub hovered: bool,
    pub focused: bool,
    pub pointer: Option<Pos2>,
    pub modifiers: Modifiers,
}

impl ForwardedInput {
    pub fn inputs(&self) -> impl Iterator<Item = &Event> {
        self.events.iter().filter_map(|event| match event {
            Forwarded::Input(event) => Some(event),
            _ => None,
        })
    }

    pub fn pointer_events(&self) -> impl Iterator<Item = &PointerEvent> {
        self.events.iter().filter_map(|event| match event {
            Forwarded::Pointer { event, .. } => Some(event),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Replayed {
    pub input: Input,
    pub time: Option<Instant>,
    pub over: Option<NodeId>,
}

impl From<Input> for Replayed {
    fn from(input: Input) -> Self {
        Self {
            input,
            time: None,
            over: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    Own,
    Other { node: NodeId, at: Pos2 },
    Away,
}

/// What modifiers a program last heard, shared by every `Replay` that feeds it: a program fed
/// through several sinks (an app's views) keeps one set of modifiers for all of them. It starts
/// as none held, as a program does.
#[derive(Clone, Debug)]
pub struct Heard(Rc<Cell<Option<Modifiers>>>);

impl Default for Heard {
    fn default() -> Self {
        Self(Rc::new(Cell::new(Some(Modifiers::NONE))))
    }
}

impl Heard {
    /// Forgets what the program heard, so the next press or key sends its modifiers again: a
    /// program that loses the keyboard lets go of its modifiers, and a restarted one starts over.
    pub fn forget(&self) {
        self.0.set(None);
    }

    /// Like `tell`, but for a view that has just got the keyboard: where the program let go of
    /// its modifiers (`forget`) and none are held, there is nothing to say.
    fn restate(&self, modifiers: Modifiers) -> bool {
        let before = self.0.replace(Some(modifiers));
        before != Some(modifiers) && !(before.is_none() && modifiers == Modifiers::NONE)
    }

    fn tell(&self, modifiers: Modifiers) -> bool {
        self.0.replace(Some(modifiers)) != Some(modifiers)
    }
}

/// Turns what one forwarding sink was given into the inputs of the program behind it. Modifiers
/// are sent only when they change from what the program last heard (`Heard`).
#[derive(Default)]
pub struct Replay {
    focused: bool,
    heard: Heard,
    pressed: u32,
    away: bool,
    /// The modifiers the latest input held, which a view gaining the keyboard is told.
    held: Modifiers,
}

impl Replay {
    /// A replay sharing with the program's other sinks what modifiers it heard, which counts the
    /// program as already knowing whether it has the keyboard (`focused`).
    pub fn hearing(heard: Heard, focused: bool) -> Self {
        Self {
            heard,
            focused,
            ..Self::default()
        }
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    /// The input that tells a view that has just got the keyboard which modifiers are held now,
    /// if the program does not know: modifiers are state, so it hears "these are held", never
    /// the presses that made them so.
    pub fn modifiers_now(&mut self) -> Option<Input> {
        self.heard
            .restate(self.held)
            .then_some(Input::Modifiers(self.held))
    }

    /// Counts no pointer button as held: the program heard none go down, so it must hear none
    /// come up.
    pub fn forget_presses(&mut self) {
        self.pressed = 0;
        self.away = false;
    }

    pub fn replay(
        &mut self,
        input: &ForwardedInput,
        zoom: f32,
        aim: impl Fn(Option<Over>, Pos2) -> Aim,
    ) -> Vec<Replayed> {
        let zoom = match zoom.is_finite() && zoom > 0.0 {
            true => zoom,
            false => 1.0,
        };
        let mut out = Vec::new();
        self.held = input.modifiers;
        for event in &input.events {
            match event {
                Forwarded::Pointer { event, over } => {
                    let aimed = match event.phase {
                        PointerPhase::Move | PointerPhase::Down(_) | PointerPhase::Up(_) => {
                            aim(*over, event.pos)
                        }
                        _ => Aim::Own,
                    };
                    let held = match event.pointer {
                        PointerId::Touch(_) => false,
                        _ => self.pressed > 0 || matches!(event.phase, PointerPhase::Up(_)),
                    };
                    match aimed {
                        Aim::Away if held => {
                            if !std::mem::replace(&mut self.away, true) {
                                out.push(Input::PointerLeft.into());
                            }
                            self.pointer(event, input.rect, zoom, None, &mut out);
                        }
                        Aim::Other { node, at } => {
                            self.away = false;
                            let aimed = PointerEvent {
                                pos: at,
                                predicted: None,
                                ..*event
                            };
                            self.pointer(&aimed, Rect::ZERO, 1.0, Some(node), &mut out);
                        }
                        _ => {
                            self.away = false;
                            self.pointer(event, input.rect, zoom, None, &mut out);
                        }
                    }
                }
                Forwarded::Left => out.push(Input::PointerLeft.into()),
                // An app's runner hears intercepted keys itself (`Input::InterceptedKey`).
                Forwarded::KeyTaken => {}
                Forwarded::Focus(focused) | Forwarded::Input(Event::Focus(focused)) => {
                    if self.focused != *focused {
                        self.focused = *focused;
                        // Whatever the program heard of its modifiers was for the view that had
                        // the keyboard: the one that gets it is told them as its state.
                        self.heard.forget();
                        out.push(Input::Focus(*focused).into());
                        if *focused {
                            out.extend(self.modifiers_now().map(Replayed::from));
                        }
                    }
                }
                Forwarded::Input(event) => self.input(event, &mut out),
            }
        }
        out
    }

    pub fn intercepted(&mut self, press: Option<KeyPress>, modifiers: Modifiers) -> Vec<Input> {
        let mut out = Vec::new();
        self.modifiers(modifiers, &mut out);
        let mut out: Vec<Input> = out.into_iter().map(|replayed| replayed.input).collect();
        if let Some(press) = press
            && press.key != crate::input::Key::Unidentified
        {
            out.push(Input::InterceptedKey {
                key: press.key,
                pressed: press.pressed,
                repeat: press.repeat,
            });
        }
        out
    }

    fn pointer(
        &mut self,
        event: &PointerEvent,
        rect: Rect,
        zoom: f32,
        over: Option<NodeId>,
        out: &mut Vec<Replayed>,
    ) {
        let at = |pos: Pos2| Pos2::ZERO + (pos - rect.min) / zoom;
        let pos = at(event.pos);
        let timed = |input: Input| Replayed {
            input,
            time: Some(event.time),
            over,
        };
        if let PointerId::Touch(id) = event.pointer {
            let phase = match event.phase {
                PointerPhase::Down(_) => TouchPhase::Start,
                PointerPhase::Move => TouchPhase::Move,
                PointerPhase::Up(_) => TouchPhase::End,
                PointerPhase::Cancel => TouchPhase::Cancel,
                _ => return,
            };
            out.push(timed(Input::Touch {
                id,
                phase,
                pos,
                force: event.force,
            }));
        } else if event.pointer == PointerId::Pen {
            let phase = match event.phase {
                PointerPhase::Move => PenPhase::Move,
                PointerPhase::Down(button) => PenPhase::Down(button),
                PointerPhase::Up(button) => PenPhase::Up(button),
                PointerPhase::Cancel | PointerPhase::Gone => PenPhase::Gone,
                _ => return,
            };
            if matches!(phase, PenPhase::Down(_) | PenPhase::Up(_)) {
                self.modifiers(event.modifiers, out);
            }
            out.push(timed(Input::Pen {
                phase,
                pos,
                pen: event.pen.unwrap_or_default(),
            }));
        } else {
            match event.phase {
                PointerPhase::Move => out.push(timed(match event.rested {
                    true => Input::PointerRested(pos),
                    false => Input::PointerMoved(pos),
                })),
                PointerPhase::Down(button) | PointerPhase::Up(button) => {
                    let pressed = matches!(event.phase, PointerPhase::Down(_));
                    self.pressed = match pressed {
                        true => self.pressed.saturating_add(1),
                        false => self.pressed.saturating_sub(1),
                    };
                    self.modifiers(event.modifiers, out);
                    out.push(timed(Input::PointerButton {
                        pos,
                        button,
                        pressed,
                    }));
                }
                PointerPhase::Scroll { delta, notches } => {
                    self.modifiers(event.modifiers, out);
                    let scroll = match notches {
                        Some(notches) => ScrollDelta::Lines(notches),
                        None => ScrollDelta::Pixels(delta / zoom),
                    };
                    out.push(Input::Scroll(scroll).into());
                }
                PointerPhase::ScrollStarted => out.push(Input::ScrollStarted.into()),
                PointerPhase::ScrollEnded { .. } => out.push(Input::ScrollEnded.into()),
                PointerPhase::Zoom(factor) => out.push(Input::Zoom(factor).into()),
                PointerPhase::Touchpad(gesture) => {
                    out.push(Input::Touchpad(gesture.scaled(1.0 / zoom)).into())
                }
                PointerPhase::Cancel | PointerPhase::Gone => {
                    if std::mem::take(&mut self.pressed) > 0 {
                        out.push(timed(Input::PointersCancelled));
                    }
                }
            }
        }
        if let (PointerPhase::Move, Some(predicted)) = (event.phase, event.predicted) {
            out.push(Input::Predicted(at(predicted)).into());
        }
    }

    fn input(&mut self, event: &Event, out: &mut Vec<Replayed>) {
        let input = match event {
            Event::PointerMotion(delta) => Input::PointerMotion(*delta),
            Event::Key { press, evdev, text } => {
                self.modifiers(press.modifiers, out);
                Input::Key(Keystroke {
                    key: Some(press.key),
                    pressed: press.pressed,
                    repeat: press.repeat,
                    evdev: *evdev,
                    text: text.clone(),
                })
            }
            Event::Modifiers(modifiers) => return self.modifiers(*modifiers, out),
            Event::Text(text) => Input::Text(text.clone()),
            Event::Paste(pasted) => Input::Paste(pasted.clone()),
            Event::Ime(ime) => Input::Ime(ime.clone()),
            _ => return,
        };
        out.push(input.into());
    }

    fn modifiers(&mut self, modifiers: Modifiers, out: &mut Vec<Replayed>) {
        if self.heard.tell(modifiers) {
            out.push(Input::Modifiers(modifiers).into());
        }
    }
}

/// Turns what a router outside beui delivered to one surface (a program window a root guest
/// places, which be-host's `RootShell` hands its compositor) into what a forwarding catcher
/// there would have caught: the inverse of `Replay`. Positions stay in the surface's points,
/// which the forwarded input's `rect` starts at the origin of.
#[derive(Default)]
pub struct Catch {
    /// Where the pointer was last, for what it says where it is no more (a wheel turning).
    pointer: Option<Pos2>,
    modifiers: Modifiers,
    focused: bool,
}

impl Catch {
    /// What `inputs` (each with when it was sampled) come to for a catcher `size` points big in
    /// `window`.
    pub fn caught(
        &mut self,
        inputs: impl IntoIterator<Item = (Input, Instant)>,
        window: crate::window::WindowId,
        size: be_lib_geometry::Vec2,
    ) -> ForwardedInput {
        let mut events = Vec::new();
        for (input, time) in inputs {
            self.catch(input, time, window, &mut events);
        }
        ForwardedInput {
            events,
            rect: Rect::from_min_size(Pos2::ZERO, size),
            hovered: self.pointer.is_some(),
            focused: self.focused,
            pointer: self.pointer,
            modifiers: self.modifiers,
        }
    }

    fn catch(
        &mut self,
        input: Input,
        time: Instant,
        window: crate::window::WindowId,
        events: &mut Vec<Forwarded>,
    ) {
        let modifiers = self.modifiers;
        let pointer = |pointer: PointerId, phase: PointerPhase, pos: Pos2| Forwarded::Pointer {
            event: PointerEvent {
                pointer,
                window,
                phase,
                pos,
                modifiers,
                // A press counts once: a compositor counts clicks itself, from their times
                // and places, as be-host-wayland's does.
                clicks: u32::from(matches!(phase, PointerPhase::Down(_))),
                force: None,
                pen: None,
                predicted: None,
                time,
                source: 0,
                rested: false,
            },
            over: None,
        };
        let at = self.pointer.unwrap_or(Pos2::ZERO);
        let event = match input {
            Input::Focus(focused) => {
                self.focused = focused;
                Forwarded::Focus(focused)
            }
            Input::Modifiers(held) => {
                self.modifiers = held;
                Forwarded::Input(Event::Modifiers(held))
            }
            Input::Key(Keystroke {
                key,
                pressed,
                repeat,
                evdev,
                text,
            }) => Forwarded::Input(Event::Key {
                press: KeyPress {
                    key: key.unwrap_or(crate::input::Key::Unidentified),
                    pressed,
                    repeat,
                    modifiers,
                },
                evdev,
                text,
            }),
            Input::Text(text) => Forwarded::Input(Event::Text(text)),
            Input::Paste(pasted) => Forwarded::Input(Event::Paste(pasted)),
            Input::Ime(ime) => Forwarded::Input(Event::Ime(ime)),
            Input::PointerMotion(delta) => Forwarded::Input(Event::PointerMotion(delta)),
            // A key the host took for itself while this had the keyboard: the catcher hears its
            // press was taken, as beui tells one (`Replay::intercepted` goes the other way).
            Input::InterceptedKey {
                pressed: true,
                repeat: false,
                ..
            } => Forwarded::KeyTaken,
            // Where the pointer will be, on the move it follows, as beui's pointer event
            // carries it.
            Input::Predicted(at) => {
                if let Some(Forwarded::Pointer { event, .. }) = events.last_mut()
                    && event.phase == PointerPhase::Move
                {
                    event.predicted = Some(at);
                }
                return;
            }
            Input::PointerMoved(pos) | Input::PointerRested(pos) => {
                self.pointer = Some(pos);
                let mut moved = pointer(PointerId::Mouse, PointerPhase::Move, pos);
                if let (Forwarded::Pointer { event, .. }, Input::PointerRested(_)) =
                    (&mut moved, &input)
                {
                    event.rested = true;
                }
                moved
            }
            Input::PointerLeft => {
                self.pointer = None;
                Forwarded::Left
            }
            Input::PointersCancelled => pointer(PointerId::Mouse, PointerPhase::Cancel, at),
            Input::PointerButton {
                pos,
                button,
                pressed,
            } => {
                self.pointer = Some(pos);
                let phase = match pressed {
                    true => PointerPhase::Down(button),
                    false => PointerPhase::Up(button),
                };
                pointer(PointerId::Mouse, phase, pos)
            }
            Input::ScrollStarted => pointer(PointerId::Mouse, PointerPhase::ScrollStarted, at),
            Input::Scroll(delta) => {
                let notches = match delta {
                    ScrollDelta::Lines(notches) => Some(notches),
                    _ => None,
                };
                let delta = crate::input::scrolled(delta, 1.0);
                pointer(
                    PointerId::Mouse,
                    PointerPhase::Scroll { delta, notches },
                    at,
                )
            }
            Input::ScrollEnded => pointer(
                PointerId::Mouse,
                PointerPhase::ScrollEnded {
                    fling: be_lib_geometry::Vec2::ZERO,
                },
                at,
            ),
            Input::Zoom(factor) => pointer(PointerId::Mouse, PointerPhase::Zoom(factor), at),
            Input::Touchpad(gesture) => {
                pointer(PointerId::Mouse, PointerPhase::Touchpad(gesture), at)
            }
            Input::Touch {
                id,
                phase,
                pos,
                force,
            } => {
                let phase = match phase {
                    TouchPhase::Start => PointerPhase::Down(crate::input::PointerButton::Primary),
                    TouchPhase::Move => PointerPhase::Move,
                    TouchPhase::End => PointerPhase::Up(crate::input::PointerButton::Primary),
                    TouchPhase::Cancel => PointerPhase::Cancel,
                };
                let mut touched = pointer(PointerId::Touch(id), phase, pos);
                if let Forwarded::Pointer { event, .. } = &mut touched {
                    event.force = force;
                }
                touched
            }
            Input::Pen { phase, pos, pen } => {
                let phase = match phase {
                    PenPhase::Move => PointerPhase::Move,
                    PenPhase::Down(button) => PointerPhase::Down(button),
                    PenPhase::Up(button) => PointerPhase::Up(button),
                    PenPhase::Gone => PointerPhase::Gone,
                };
                let mut penned = pointer(PointerId::Pen, phase, pos);
                if let Forwarded::Pointer { event, .. } = &mut penned {
                    event.pen = Some(pen);
                }
                penned
            }
            // What a program window has no use for: a back gesture, an intercepted key's
            // repeat or release and the taps a host intercepted for itself, a paste in pieces,
            // the pointer's lock ending.
            _ => return,
        };
        events.push(event);
    }
}

#[cfg(test)]
mod tests;
