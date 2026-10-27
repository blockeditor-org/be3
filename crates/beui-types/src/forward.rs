use std::time::Instant;

use geometry::{Pos2, Rect};

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

#[derive(Default)]
pub struct Replay {
    focused: bool,
    modifiers: Modifiers,
    pressed: u32,
    away: bool,
}

impl Replay {
    pub fn focused(&self) -> bool {
        self.focused
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
                Forwarded::Focus(focused) | Forwarded::Input(Event::Focus(focused)) => {
                    if self.focused != *focused {
                        self.focused = *focused;
                        out.push(Input::Focus(*focused).into());
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
                PointerPhase::Move => out.push(timed(Input::PointerMoved(pos))),
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
                PointerPhase::Scroll(delta) => {
                    self.modifiers(event.modifiers, out);
                    out.push(Input::Scroll(ScrollDelta::Pixels(delta / zoom)).into());
                }
                PointerPhase::ScrollStarted => out.push(Input::ScrollStarted.into()),
                PointerPhase::ScrollEnded { .. } => out.push(Input::ScrollEnded.into()),
                PointerPhase::Zoom(factor) => out.push(Input::Zoom(factor).into()),
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
        if self.modifiers != modifiers {
            self.modifiers = modifiers;
            out.push(Input::Modifiers(modifiers).into());
        }
    }
}
