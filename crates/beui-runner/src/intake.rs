use std::time::Instant;

use beui_types::HostTransform;
use beui_types::ScrollDelta;
use beui_types::input::enqueue;
use beui_types::{BackGesture, Event, FileDrag, PenPhase};
use beui_types::{Input, Key, KeyPress, Keystroke, Modifiers, PointerButton, TouchId, TouchPhase};
use beui_types::{Pos2, Vec2};
use beui_types::{WindowId, WindowReport};

pub use beui_types::input::{PINCH_SPEED, SCROLL_LINE, SCROLL_PAGE, scrolled};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The units an `Incoming` input is in. Logical input is converted with the scale
/// the frame lays out at, so no platform reads its scale at event time.
pub enum Units {
    Physical,
    Logical,
    Points,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    Input {
        units: Units,
        window: WindowId,
        input: Input,
    },
    Layer {
        layer: u64,
        input: Input,
    },
    FileDrag(FileDrag),
    /// Pushed just after the event it stamps; becomes `PointerEvent::time` and
    /// drives fling velocity. An event pushed without one happens at the frame
    /// clock's `now`, so tests step time by stepping frames.
    Time(Instant),
    Window(WindowReport),
    Controller(super::Controller),
}

impl Incoming {
    pub fn new(units: Units, input: Input) -> Self {
        Self::Input {
            units,
            window: WindowId::MAIN,
            input,
        }
    }

    pub fn logical(input: Input) -> Self {
        Self::new(Units::Logical, input)
    }

    pub fn points(input: Input) -> Self {
        Self::new(Units::Points, input)
    }

    pub fn layer(layer: u64, input: Input) -> Self {
        Self::Layer { layer, input }
    }

    pub fn in_window(mut self, id: WindowId) -> Self {
        if let Self::Input { window, .. } = &mut self {
            *window = id;
        }
        self
    }
}

fn from_host(input: Input, to_host: HostTransform) -> Input {
    let at = |pos: Pos2| to_host.inverse_point(pos);
    match input {
        Input::PointerMoved(pos) => Input::PointerMoved(at(pos)),
        Input::PointerRested(pos) => Input::PointerRested(at(pos)),
        Input::PointerButton {
            pos,
            button,
            pressed,
        } => Input::PointerButton {
            pos: at(pos),
            button,
            pressed,
        },
        Input::PointerMotion(delta) => Input::PointerMotion(to_host.inverse_vec(delta)),
        Input::Touch {
            id,
            phase,
            pos,
            force,
        } => Input::Touch {
            id,
            phase,
            pos: at(pos),
            force,
        },
        Input::Pen { phase, pos, pen } => Input::Pen {
            phase,
            pos: at(pos),
            pen,
        },
        Input::Predicted(pos) => Input::Predicted(at(pos)),
        other => other,
    }
}

impl From<WindowReport> for Incoming {
    fn from(report: WindowReport) -> Self {
        Self::Window(report)
    }
}

impl From<super::Controller> for Incoming {
    fn from(controller: super::Controller) -> Self {
        Self::Controller(controller)
    }
}

impl From<FileDrag> for Incoming {
    fn from(drag: FileDrag) -> Self {
        Self::FileDrag(drag)
    }
}

impl From<Input> for Incoming {
    fn from(input: Input) -> Self {
        Self::new(Units::Physical, input)
    }
}

pub(crate) struct Intake {
    pub(crate) incoming: Vec<Incoming>,
    pub(crate) reports: Vec<WindowReport>,
    pub(crate) every_sample: Vec<WindowId>,
    modifiers: Modifiers,
    pointer: Pos2,
    window: WindowId,
    held: u8,
    left: bool,
    emulated_touch: bool,
    paste_pieces: Vec<u8>,
}

impl Default for Intake {
    fn default() -> Self {
        Self {
            incoming: Vec::new(),
            reports: Vec::new(),
            every_sample: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer: Pos2::ZERO,
            window: WindowId::MAIN,
            held: 0,
            left: false,
            emulated_touch: false,
            paste_pieces: Vec::new(),
        }
    }
}

pub(crate) struct Drain<'a> {
    pub pixels_per_point: f32,
    pub native_pixels_per_point: f32,
    pub touch_emulation: bool,
    pub window_size: &'a dyn Fn(WindowId) -> Option<Vec2>,
    pub layer_target: &'a dyn Fn(u64) -> Option<(WindowId, HostTransform)>,
}

impl Intake {
    pub(crate) fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub(crate) fn drain(&mut self, drain: &mut Drain<'_>, events: &mut Vec<Event>) {
        if !drain.touch_emulation && std::mem::take(&mut self.emulated_touch) {
            events.push(self.touch(TouchPhase::Cancel));
        }
        let mut queue = Vec::new();
        for incoming in std::mem::take(&mut self.incoming) {
            match incoming {
                Incoming::Time(at) => {
                    if let Some(last) = queue.last_mut()
                        && let Queued {
                            time: time @ None, ..
                        } = last
                    {
                        *time = Some(at);
                    }
                }
                Incoming::FileDrag(drag) => queue.push(Queued {
                    entry: Entry::FileDrag(drag),
                    time: None,
                }),
                Incoming::Window(report) => self.reports.push(report),
                Incoming::Controller(_) => {}
                Incoming::Layer { layer, input } => {
                    let Some((window, to_host)) = (drain.layer_target)(layer) else {
                        continue;
                    };
                    enqueue(
                        &mut queue,
                        Queued {
                            entry: Entry::Layer {
                                layer,
                                window,
                                input: from_host(input, to_host),
                            },
                            time: None,
                        },
                        self.every_sample.contains(&window),
                    );
                }
                Incoming::Input {
                    units,
                    window,
                    input,
                } => enqueue(
                    &mut queue,
                    Queued {
                        entry: Entry::Input {
                            units,
                            window,
                            input,
                        },
                        time: None,
                    },
                    self.every_sample.contains(&window),
                ),
            }
        }
        for Queued { entry, time } in queue {
            let before = events.len();
            match entry {
                Entry::FileDrag(drag) => events.push(match drag {
                    FileDrag::Hovered => Event::FileHovered,
                    FileDrag::Cancelled => Event::FileHoverCancelled,
                    FileDrag::Dropped(file) => Event::FileDropped(file),
                }),
                Entry::Input {
                    units,
                    window,
                    input,
                } => {
                    if window != self.window && points_into_window(&input) {
                        self.window = window;
                        events.push(Event::PointerWindow(window));
                    }
                    let scale = match units {
                        Units::Physical => drain.pixels_per_point,
                        Units::Logical => drain.pixels_per_point / drain.native_pixels_per_point,
                        Units::Points => 1.0,
                    };
                    self.input(input, scale, drain, events);
                }
                Entry::Layer { window, input, .. } => {
                    if window != self.window && points_into_window(&input) {
                        self.window = window;
                        events.push(Event::PointerWindow(window));
                    }
                    self.input(input, 1.0, drain, events);
                }
            }
            if let Some(at) = time
                && events.len() > before
            {
                events.push(Event::Time(at));
            }
        }
    }

    fn input(&mut self, input: Input, scale: f32, drain: &Drain<'_>, events: &mut Vec<Event>) {
        let points = |pos: Pos2| Pos2::new(pos.x / scale, pos.y / scale);
        match input {
            Input::Key(stroke) => self.key(stroke, events),
            Input::Scroll(scroll) => {
                let delta = scrolled(scroll, scale);
                let notches = match scroll {
                    ScrollDelta::Lines(lines) => Some(lines),
                    ScrollDelta::Pixels(_) | ScrollDelta::Pages(_) => None,
                };
                if delta != Vec2::ZERO {
                    events.push(Event::Scroll { delta, notches });
                }
            }
            Input::Modifiers(modifiers) => self.set_modifiers(modifiers, events),
            Input::PointerMoved(pos) | Input::PointerRested(pos) => {
                self.pointer = points(pos);
                self.back_inside(drain);
                if !drain.touch_emulation {
                    events.push(match input {
                        Input::PointerRested(_) => Event::PointerRested(self.pointer),
                        _ => Event::PointerMoved(self.pointer),
                    });
                } else if self.emulated_touch {
                    events.push(self.touch(TouchPhase::Move));
                }
            }
            Input::PointerButton {
                pos,
                button,
                pressed,
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
            Input::PointerLeft => match self.emulated_touch || self.held != 0 {
                true => {
                    self.left = true;
                    events.push(Event::PointerOutside);
                }
                false => events.push(Event::PointerGone),
            },
            Input::PointerMotion(delta) => events.push(Event::PointerMotion(delta / scale)),
            Input::Touch {
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
            Input::Pen { phase, pos, pen } => events.push(Event::Pen {
                phase,
                pos: points(pos),
                pen,
            }),
            Input::Predicted(pos) => events.push(Event::Predicted(points(pos))),
            Input::PointersCancelled => {
                self.held = 0;
                self.left = false;
                if std::mem::take(&mut self.emulated_touch) {
                    events.push(self.touch(TouchPhase::Cancel));
                }
                events.push(Event::PointersCancelled);
            }
            Input::Focus(false) => {
                self.set_modifiers(Modifiers::NONE, events);
                events.push(Event::Focus(false));
            }
            Input::Focus(true) => events.push(Event::Focus(true)),
            Input::PointerUnlocked => events.push(Event::PointerUnlocked),
            Input::InterceptedKey {
                key,
                pressed,
                repeat,
            } => events.push(Event::InterceptedKey(KeyPress {
                key,
                pressed,
                repeat,
                modifiers: self.modifiers,
            })),
            Input::InterceptedTap(key) => events.push(Event::InterceptedTap(key)),
            Input::Text(text) => events.push(Event::Text(text)),
            Input::PastePiece(piece) => self.paste_pieces.extend(piece.data),
            Input::Paste(mut pasted) => {
                if !self.paste_pieces.is_empty() {
                    let mut joined = std::mem::take(&mut self.paste_pieces);
                    joined.append(&mut pasted.data);
                    pasted.data = joined;
                }
                events.push(Event::Paste(pasted));
            }
            Input::Ime(ime) => events.push(Event::Ime(ime)),
            Input::ScrollStarted => events.push(Event::ScrollStarted),
            Input::ScrollEnded => events.push(Event::ScrollEnded),
            Input::Zoom(factor) => events.push(Event::Zoom(factor)),
            Input::Touchpad(gesture) => events.push(Event::Touchpad(gesture.scaled(1.0 / scale))),
            Input::Back(gesture) => events.push(Event::Back(gesture)),
        }
    }

    fn back_inside(&mut self, drain: &Drain<'_>) {
        if self.left
            && (drain.window_size)(self.window).is_some_and(|size| {
                let pos = self.pointer;
                pos.x >= 0.0 && pos.y >= 0.0 && pos.x < size.x && pos.y < size.y
            })
        {
            self.left = false;
        }
    }

    fn key(&mut self, stroke: Keystroke, events: &mut Vec<Event>) {
        let Keystroke {
            key,
            pressed,
            repeat,
            evdev,
            text,
        } = stroke;
        let modifiers = self.modifiers;
        let text = text.filter(|text| {
            pressed
                && !modifiers.command()
                && !text.is_empty()
                && !text.chars().any(char::is_control)
        });
        let Some(key) = key.or(evdev.map(|_| Key::Unidentified)) else {
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
            evdev,
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

struct Queued {
    entry: Entry,
    time: Option<Instant>,
}

enum Entry {
    Input {
        units: Units,
        window: WindowId,
        input: Input,
    },
    Layer {
        layer: u64,
        window: WindowId,
        input: Input,
    },
    FileDrag(FileDrag),
}

impl beui_types::input::Queued for Queued {
    fn input(&self) -> Option<&Input> {
        match &self.entry {
            Entry::Input { input, .. } | Entry::Layer { input, .. } => Some(input),
            Entry::FileDrag(_) => None,
        }
    }

    fn input_mut(&mut self) -> Option<&mut Input> {
        match &mut self.entry {
            Entry::Input { input, .. } | Entry::Layer { input, .. } => Some(input),
            Entry::FileDrag(_) => None,
        }
    }

    fn joins(&self, later: &Self) -> bool {
        match (&self.entry, &later.entry) {
            (
                Entry::Input { units, window, .. },
                Entry::Input {
                    units: later_units,
                    window: later_window,
                    ..
                },
            ) => units == later_units && window == later_window,
            (Entry::Layer { layer, .. }, Entry::Layer { layer: later, .. }) => layer == later,
            _ => false,
        }
    }
}

/// Whether an input ends something the program may be in the middle of: a
/// press, a drag, a gesture, focus or a composition. While the window has no
/// surface its frames do not run; these wait for the next frame that does, and
/// everything else is dropped (`Runner::frame`).
pub(crate) fn ends_something(incoming: &Incoming) -> bool {
    match incoming {
        Incoming::Input { input, .. } | Incoming::Layer { input, .. } => input_ends(input),
        Incoming::FileDrag(drag) => !matches!(drag, FileDrag::Hovered),
        Incoming::Window(_) => true,
        Incoming::Time(_) | Incoming::Controller(_) => false,
    }
}

fn input_ends(input: &Input) -> bool {
    match input {
        Input::Focus(_)
        | Input::Modifiers(_)
        | Input::Text(_)
        | Input::Paste(_)
        | Input::PastePiece(_)
        | Input::Ime(_)
        | Input::PointerLeft
        | Input::PointersCancelled
        | Input::PointerUnlocked
        | Input::ScrollEnded => true,
        Input::Key(keystroke) => !keystroke.pressed,
        Input::PointerButton { pressed, .. } => !pressed,
        Input::Touch { phase, .. } => matches!(phase, TouchPhase::End | TouchPhase::Cancel),
        Input::Pen { phase, .. } => matches!(phase, PenPhase::Up(_) | PenPhase::Gone),
        Input::Back(gesture) => matches!(gesture, BackGesture::Cancelled | BackGesture::Invoked),
        _ => false,
    }
}

/// `ends_something` for input already read into events.
pub(crate) fn event_ends_something(event: &Event) -> bool {
    match event {
        Event::Focus(_)
        | Event::Modifiers(_)
        | Event::Text(_)
        | Event::Paste(_)
        | Event::Ime(_)
        | Event::PointerGone
        | Event::PointersCancelled
        | Event::PointerUnlocked
        | Event::PointerOutside
        | Event::ScrollEnded
        | Event::FileHoverCancelled
        | Event::FileDropped(_) => true,
        Event::Key { press, .. } => !press.pressed,
        Event::PointerButton { pressed, .. } => !pressed,
        Event::Touch { phase, .. } => matches!(phase, TouchPhase::End | TouchPhase::Cancel),
        Event::Pen { phase, .. } => matches!(phase, PenPhase::Up(_) | PenPhase::Gone),
        Event::Back(gesture) => matches!(gesture, BackGesture::Cancelled | BackGesture::Invoked),
        _ => false,
    }
}

fn points_into_window(input: &Input) -> bool {
    matches!(
        input,
        Input::PointerMoved(_)
            | Input::PointerRested(_)
            | Input::PointerButton { .. }
            | Input::PointerLeft
            | Input::Touch { .. }
            | Input::Pen { .. }
            | Input::Predicted(_)
            | Input::Scroll(_)
            | Input::ScrollStarted
            | Input::ScrollEnded
            | Input::Zoom(_)
            | Input::Touchpad(_)
    )
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
