use crate::geometry::{Pos2, Vec2};
use crate::input::{Event, Key, KeyPress, Modifiers, PointerButton, TouchId, TouchPhase};

pub const SCROLL_LINE: f32 = 40.0;
pub const SCROLL_PAGE: f32 = 800.0;
pub const PINCH_SPEED: f32 = 0.01;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keystroke {
    pub key: Option<Key>,
    pub pressed: bool,
    pub repeat: bool,
    pub code: Option<u32>,
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    Pixels(Vec2),
    Lines(Vec2),
    Pages(Vec2),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Event(Event),
    Key(Keystroke),
    Scroll(ScrollDelta),
    Pinch(ScrollDelta),
    Logical(Box<Input>),
}

impl Input {
    pub fn logical(input: impl Into<Input>) -> Self {
        Self::Logical(Box::new(input.into()))
    }
}

impl From<Event> for Input {
    fn from(event: Event) -> Self {
        Self::Event(event)
    }
}

impl From<Keystroke> for Input {
    fn from(stroke: Keystroke) -> Self {
        Self::Key(stroke)
    }
}

impl From<ScrollDelta> for Input {
    fn from(delta: ScrollDelta) -> Self {
        Self::Scroll(delta)
    }
}

pub(crate) struct Intake {
    pub(crate) incoming: Vec<Input>,
    modifiers: Modifiers,
    pointer: Pos2,
    held: u8,
    left: bool,
    emulated_touch: bool,
}

impl Default for Intake {
    fn default() -> Self {
        Self {
            incoming: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer: Pos2::ZERO,
            held: 0,
            left: false,
            emulated_touch: false,
        }
    }
}

pub(crate) struct Drain {
    pub pixels_per_point: f32,
    pub native_pixels_per_point: f32,
    pub touch_emulation: bool,
}

impl Intake {
    pub(crate) fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub(crate) fn drain(&mut self, drain: &mut Drain, events: &mut Vec<Event>) {
        if !drain.touch_emulation && std::mem::take(&mut self.emulated_touch) {
            events.push(self.touch(TouchPhase::Cancel));
        }
        for incoming in std::mem::take(&mut self.incoming) {
            self.input(incoming, drain.pixels_per_point, drain, events);
        }
    }

    fn input(&mut self, input: Input, scale: f32, drain: &Drain, events: &mut Vec<Event>) {
        match input {
            Input::Event(event) => self.event(event, scale, drain, events),
            Input::Key(stroke) => self.key(stroke, events),
            Input::Scroll(delta) => {
                let delta = points(delta, scale);
                if delta != Vec2::ZERO {
                    events.push(Event::Scroll(delta));
                }
            }
            Input::Pinch(delta) => {
                let pinched = points(delta, scale).y;
                if pinched != 0.0 {
                    events.push(Event::Zoom((pinched * PINCH_SPEED).exp()));
                }
            }
            Input::Logical(input) => {
                let logical = drain.pixels_per_point / drain.native_pixels_per_point;
                self.input(*input, logical, drain, events);
            }
        }
    }

    fn event(&mut self, event: Event, scale: f32, drain: &Drain, events: &mut Vec<Event>) {
        let points = |pos: Pos2| Pos2::new(pos.x / scale, pos.y / scale);
        match event {
            Event::Modifiers(modifiers) => self.set_modifiers(modifiers, events),
            Event::PointerMoved(pos) => {
                self.pointer = points(pos);
                if !drain.touch_emulation {
                    events.push(Event::PointerMoved(self.pointer));
                } else if self.emulated_touch {
                    events.push(self.touch(TouchPhase::Move));
                }
            }
            Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => {
                self.pointer = points(pos);
                let bit = button_bit(button);
                match pressed {
                    true => self.held |= bit,
                    false => self.held &= !bit,
                }
                let released_outside = !pressed && self.left && self.held == 0;
                if drain.touch_emulation {
                    if button == PointerButton::Primary && pressed != self.emulated_touch {
                        self.emulated_touch = pressed;
                        events.push(self.touch(match pressed {
                            true => TouchPhase::Start,
                            false => TouchPhase::End,
                        }));
                    }
                } else {
                    events.push(Event::PointerButton {
                        pos: self.pointer,
                        button,
                        pressed,
                        modifiers: self.modifiers,
                    });
                }
                if released_outside && !self.emulated_touch {
                    self.left = false;
                    events.push(Event::PointerGone);
                }
            }
            Event::PointerGone => match self.emulated_touch || self.held != 0 {
                true => self.left = true,
                false => events.push(Event::PointerGone),
            },
            Event::PointerMotion(delta) => events.push(Event::PointerMotion(delta / scale)),
            Event::Scroll(delta) => {
                self.input(
                    Input::Scroll(ScrollDelta::Pixels(delta)),
                    scale,
                    drain,
                    events,
                );
            }
            Event::Touch {
                id,
                phase,
                pos,
                force,
            } => events.push(Event::Touch {
                id,
                phase,
                pos: points(pos),
                force,
            }),
            Event::Pen {
                phase,
                pos,
                pressure,
                pen,
            } => events.push(Event::Pen {
                phase,
                pos: points(pos),
                pressure,
                pen,
            }),
            Event::Predicted(pos) => events.push(Event::Predicted(points(pos))),
            Event::Focus(false) => {
                self.held = 0;
                self.left = false;
                if std::mem::take(&mut self.emulated_touch) {
                    events.push(self.touch(TouchPhase::Cancel));
                }
                self.set_modifiers(Modifiers::NONE, events);
                events.push(Event::Focus(false));
            }
            Event::InterceptedKey(press) => events.push(Event::InterceptedKey(KeyPress {
                modifiers: self.modifiers,
                ..press
            })),
            event => events.push(event),
        }
    }

    fn key(&mut self, stroke: Keystroke, events: &mut Vec<Event>) {
        let Keystroke {
            key,
            pressed,
            repeat,
            code,
            text,
        } = stroke;
        let modifiers = self.modifiers;
        let text = text.filter(|text| {
            pressed
                && !modifiers.command()
                && !text.is_empty()
                && !text.chars().any(char::is_control)
        });
        let Some(key) = key.or(code.map(|_| Key::Unidentified)) else {
            events.extend(text.map(Event::Text));
            return;
        };
        if repeat && key.is_modifier() {
            return;
        }
        events.push(Event::Key {
            press: KeyPress {
                key,
                pressed,
                repeat,
                modifiers,
            },
            code,
            text,
        });
    }

    fn set_modifiers(&mut self, modifiers: Modifiers, events: &mut Vec<Event>) {
        if modifiers != self.modifiers {
            self.modifiers = modifiers;
            events.push(Event::Modifiers(modifiers));
        }
    }

    fn touch(&self, phase: TouchPhase) -> Event {
        Event::Touch {
            id: TouchId {
                device: 0,
                finger: 0,
            },
            phase,
            pos: self.pointer,
            force: None,
        }
    }
}

fn points(delta: ScrollDelta, scale: f32) -> Vec2 {
    match delta {
        ScrollDelta::Pixels(delta) => delta / scale,
        ScrollDelta::Lines(delta) => delta * SCROLL_LINE,
        ScrollDelta::Pages(delta) => delta * SCROLL_PAGE,
    }
}

fn button_bit(button: PointerButton) -> u8 {
    match button {
        PointerButton::Primary => 1,
        PointerButton::Secondary => 2,
        PointerButton::Middle => 4,
        PointerButton::Back => 8,
        PointerButton::Forward => 16,
        PointerButton::Other(_) => 32,
    }
}
