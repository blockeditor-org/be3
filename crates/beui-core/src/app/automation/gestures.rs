use crate::geometry::{Pos2, Vec2, vec2};
use crate::input::{
    BackEdge, BackGesture, Event, ImeEvent, Key, Modifiers, Pen, PenPhase, PointerButton, TouchId,
    TouchPhase,
};
use crate::runner::SCROLL_LINE as WHEEL_LINE;

use super::keys;

pub type Frames = Vec<Vec<Event>>;

const GESTURE_STEPS: usize = 4;
const PINCH_SPAN: f32 = 100.0;

#[derive(Default)]
pub struct Gestures {
    pub(crate) held: Modifiers,
    pub(crate) pointer: Pos2,
    hovering: bool,
    pub(crate) buttons: Vec<PointerButton>,
    pub(crate) fingers: Vec<(u64, Pos2)>,
    pub(crate) pen: Option<(Pos2, bool)>,
    pub(crate) keys: Vec<Key>,
    pub(crate) composing: Option<String>,
    ime: bool,
    pub(crate) back: Option<f32>,
}

impl Gestures {
    pub fn pointer(&self) -> Pos2 {
        self.pointer
    }

    pub fn held(&self) -> Modifiers {
        self.held
    }

    pub fn click(&mut self, at: Pos2, button: PointerButton, count: usize) -> Frames {
        let mut frames = self.arrive(at);
        frames.extend((0..count).map(|_| {
            vec![
                Event::PointerMoved(at),
                self.button(at, button, true),
                self.button(at, button, false),
            ]
        }));
        frames
    }

    pub fn hover(&mut self, at: Pos2) -> Frames {
        self.pointer = at;
        self.hovering = true;
        vec![vec![Event::PointerMoved(at)]]
    }

    pub fn leave(&mut self) -> Frames {
        self.hovering = false;
        vec![vec![Event::PointerGone]]
    }

    pub fn down(&mut self, at: Pos2, button: PointerButton) -> Frames {
        let mut frames = self.arrive(at);
        self.buttons.push(button);
        frames.push(vec![Event::PointerMoved(at), self.button(at, button, true)]);
        frames
    }

    pub fn up(&mut self, at: Option<Pos2>, button: PointerButton) -> Frames {
        let at = at.unwrap_or(self.pointer);
        self.pointer = at;
        self.hovering = true;
        self.buttons.retain(|held| *held != button);
        vec![vec![
            Event::PointerMoved(at),
            self.button(at, button, false),
        ]]
    }

    pub fn drag(&mut self, from: Pos2, to: Pos2, steps: usize) -> Frames {
        let mut frames = self.arrive(from);
        self.pointer = to;
        frames.push(vec![
            Event::PointerMoved(from),
            self.button(from, PointerButton::Primary, true),
        ]);
        for step in 1..steps.max(1) {
            frames.push(vec![Event::PointerMoved(
                from + (to - from) * (step as f32 / steps as f32),
            )]);
        }
        frames.push(vec![
            Event::PointerMoved(to),
            self.button(to, PointerButton::Primary, false),
        ]);
        frames
    }

    pub fn wheel(&mut self, at: Pos2, down: i32, right: i32) -> Frames {
        self.pointer = at;
        let count = down.unsigned_abs().max(right.unsigned_abs());
        (0..count)
            .map(|tick| {
                let along = |ticks: i32| match tick < ticks.unsigned_abs() {
                    true => -(ticks.signum() as f32) * WHEEL_LINE,
                    false => 0.0,
                };
                vec![
                    Event::PointerMoved(at),
                    Event::Scroll(vec2(along(right), along(down))),
                ]
            })
            .collect()
    }

    pub fn zoom(&mut self, at: Pos2, factor: f32) -> Frames {
        self.pointer = at;
        let step = factor.powf(1.0 / GESTURE_STEPS as f32);
        (0..GESTURE_STEPS)
            .map(|_| vec![Event::PointerMoved(at), Event::Zoom(step)])
            .collect()
    }

    pub fn pinch(&mut self, center: Pos2, factor: f32) -> Frames {
        let start = PINCH_SPAN / 2.0;
        let end = start * factor;
        let fingers = |half: f32, phase: TouchPhase| {
            vec![
                touch(0, phase, center - vec2(half, 0.0)),
                touch(1, phase, center + vec2(half, 0.0)),
            ]
        };
        let mut frames = vec![fingers(start, TouchPhase::Start)];
        for step in 1..=GESTURE_STEPS {
            let half = start + (end - start) * (step as f32 / GESTURE_STEPS as f32);
            frames.push(fingers(half, TouchPhase::Move));
        }
        frames.push(fingers(end, TouchPhase::End));
        frames
    }

    pub fn scroll(&mut self, at: Pos2, by: Vec2) -> Frames {
        self.pointer = at;
        vec![vec![
            Event::PointerMoved(at),
            Event::Scroll(-by),
            Event::ScrollEnded,
        ]]
    }

    pub fn tap(&mut self, at: Pos2, finger: u64) -> Frames {
        vec![vec![
            touch(finger, TouchPhase::Start, at),
            touch(finger, TouchPhase::End, at),
        ]]
    }

    pub fn swipe(&mut self, from: Pos2, to: Pos2, finger: u64) -> Frames {
        (0..=4)
            .map(|step| {
                let at = from + (to - from) * (step as f32 / 4.0);
                let phase = match step {
                    0 => TouchPhase::Start,
                    _ => TouchPhase::Move,
                };
                vec![touch(finger, phase, at)]
            })
            .chain(std::iter::once(vec![touch(finger, TouchPhase::End, to)]))
            .collect()
    }

    pub fn touch(
        &mut self,
        phase: TouchPhase,
        fingers: &[(u64, Option<Pos2>)],
    ) -> Result<Frames, String> {
        let lifting = matches!(phase, TouchPhase::End | TouchPhase::Cancel);
        let mut events = Vec::new();
        if fingers.is_empty() {
            if !lifting {
                return Err("touch down and touch move need a TARGET".to_owned());
            }
            for (finger, pos) in std::mem::take(&mut self.fingers) {
                events.push(touch(finger, phase, pos));
            }
        }
        for (finger, target) in fingers {
            let finger = *finger;
            let held = self.fingers.iter().position(|(held, _)| *held == finger);
            let pos = match (target, held) {
                (Some(target), _) => *target,
                (None, Some(index)) if lifting => self.fingers[index].1,
                (None, _) => return Err(format!("finger {finger} names no TARGET")),
            };
            match (phase, held) {
                (TouchPhase::Start, Some(_)) => {
                    return Err(format!("finger {finger} is already down"));
                }
                (TouchPhase::Start, None) => self.fingers.push((finger, pos)),
                (_, None) => return Err(format!("finger {finger} is not down")),
                (_, Some(index)) if lifting => {
                    self.fingers.remove(index);
                }
                (_, Some(index)) => self.fingers[index].1 = pos,
            }
            events.push(touch(finger, phase, pos));
        }
        Ok(vec![events])
    }

    pub fn pen_move(&mut self, to: Pos2, pressure: Option<f32>) -> Frames {
        let down = self.pen.is_some_and(|(_, down)| down);
        let pressure = pressure.unwrap_or(if down { 0.5 } else { 0.0 });
        self.pen = Some((to, down));
        vec![vec![pen(PenPhase::Move, to, pressure)]]
    }

    pub fn pen_down(&mut self, to: Pos2, pressure: Option<f32>) -> Result<Frames, String> {
        if self.pen.is_some_and(|(_, down)| down) {
            return Err("the pen is already down".to_owned());
        }
        self.pen = Some((to, true));
        Ok(vec![vec![
            pen(PenPhase::Move, to, 0.0),
            pen(
                PenPhase::Down(PointerButton::Primary),
                to,
                pressure.unwrap_or(0.5),
            ),
        ]])
    }

    pub fn pen_up(&mut self, to: Option<Pos2>) -> Result<Frames, String> {
        let Some((last, true)) = self.pen else {
            return Err("the pen is not down".to_owned());
        };
        let to = to.unwrap_or(last);
        self.pen = Some((to, false));
        Ok(vec![vec![
            pen(PenPhase::Move, to, 0.5),
            pen(PenPhase::Up(PointerButton::Primary), to, 0.0),
        ]])
    }

    pub fn pen_draw(&mut self, from: Pos2, to: Pos2, steps: usize) -> Result<Frames, String> {
        if self.pen.is_some_and(|(_, down)| down) {
            return Err("the pen is already down".to_owned());
        }
        let steps = steps.max(1);
        let along = |step: usize| {
            let fraction = step as f32 / steps as f32;
            let pressure = 0.2 + 0.8 * (fraction * std::f32::consts::PI).sin();
            (from + (to - from) * fraction, pressure)
        };
        let mut frames = vec![vec![
            pen(PenPhase::Move, from, 0.0),
            pen(PenPhase::Down(PointerButton::Primary), from, 0.2),
        ]];
        for step in 1..=steps {
            let (at, pressure) = along(step);
            frames.push(vec![pen(PenPhase::Move, at, pressure)]);
        }
        frames.push(vec![pen(PenPhase::Up(PointerButton::Primary), to, 0.0)]);
        self.pen = Some((to, false));
        Ok(frames)
    }

    pub fn pen_leave(&mut self) -> Result<Frames, String> {
        let Some((last, down)) = self.pen else {
            return Err("the pen is not in range".to_owned());
        };
        let mut frame = Vec::new();
        if down {
            frame.push(pen(PenPhase::Up(PointerButton::Primary), last, 0.0));
        }
        frame.push(pen(PenPhase::Gone, last, 0.0));
        self.pen = None;
        Ok(vec![frame])
    }

    pub fn back(&mut self) -> Frames {
        let mut events = Vec::new();
        self.start_back(BackEdge::Left, &mut events);
        let mut frames = vec![events];
        for progress in [0.5, 1.0] {
            frames.push(vec![Event::Back(BackGesture::Progressed(progress))]);
        }
        frames.push(vec![Event::Back(BackGesture::Invoked)]);
        self.back = None;
        frames
    }

    pub fn back_from(&mut self, edge: BackEdge) -> Result<Frames, String> {
        if self.back.is_some() {
            return Err("a back gesture is already under way".to_owned());
        }
        let mut events = Vec::new();
        self.start_back(edge, &mut events);
        Ok(vec![events])
    }

    pub fn back_to(&mut self, progress: f32) -> Frames {
        let mut events = Vec::new();
        self.start_back(BackEdge::Left, &mut events);
        self.back = Some(progress);
        events.push(Event::Back(BackGesture::Progressed(progress)));
        vec![events]
    }

    pub fn back_end(&mut self, commit: bool) -> Result<Frames, String> {
        if self.back.take().is_none() {
            return Err("no back gesture is under way".to_owned());
        }
        Ok(vec![vec![Event::Back(match commit {
            true => BackGesture::Invoked,
            false => BackGesture::Cancelled,
        })]])
    }

    pub fn type_text(&mut self, text: &str) -> Frames {
        let mut events = Vec::new();
        keys::typed(text, self.held, &mut events);
        vec![events]
    }

    pub fn chord(&mut self, key: Key, modifiers: Modifiers) -> Frames {
        let mut events = Vec::new();
        keys::press_chord(key, modifiers, self.held, &mut events);
        vec![events]
    }

    pub fn chords(&mut self, chords: &[String]) -> Result<Frames, String> {
        let mut events = Vec::new();
        for chord in chords {
            keys::chord(chord, self.held, &mut events)?;
        }
        Ok(vec![events])
    }

    pub fn key(&mut self, key: Key, pressed: bool) -> Result<Frames, String> {
        let mut events = Vec::new();
        if key.is_modifier() {
            keys::set_modifier(&mut self.held, key, pressed);
            keys::modifier_event(key, pressed, self.held, &mut events);
            return Ok(vec![events]);
        }
        let held = self.keys.contains(&key);
        match (pressed, held) {
            (true, false) => self.keys.push(key),
            (false, false) => return Err(format!("{key:?} is not held down")),
            (false, true) => self.keys.retain(|down| *down != key),
            (true, true) => {}
        }
        keys::key_event(key, pressed, pressed && held, self.held, &mut events);
        Ok(vec![events])
    }

    pub fn compose(&mut self, text: String) -> Frames {
        let mut events = self.enable_ime();
        self.composing = Some(text.clone());
        events.push(Event::Ime(ImeEvent::SetComposingText(text)));
        vec![events]
    }

    pub fn commit(&mut self, text: Option<String>) -> Frames {
        let mut events = self.enable_ime();
        let composed = self.composing.take().unwrap_or_default();
        events.push(Event::Ime(ImeEvent::SetComposingText(String::new())));
        events.push(Event::Ime(ImeEvent::CommitText(text.unwrap_or(composed))));
        vec![events]
    }

    pub fn cancel_composing(&mut self) -> Frames {
        let mut events = self.enable_ime();
        self.composing = None;
        events.push(Event::Ime(ImeEvent::SetComposingText(String::new())));
        vec![events]
    }

    pub(crate) fn arrive(&mut self, at: Pos2) -> Frames {
        let arrived = self.hovering && self.pointer == at;
        self.pointer = at;
        self.hovering = true;
        match arrived {
            true => Vec::new(),
            false => vec![vec![Event::PointerMoved(at)]],
        }
    }

    pub(crate) fn button(&self, pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: self.held,
        }
    }

    fn enable_ime(&mut self) -> Vec<Event> {
        match std::mem::replace(&mut self.ime, true) {
            true => Vec::new(),
            false => vec![Event::Ime(ImeEvent::Enabled)],
        }
    }

    fn start_back(&mut self, edge: BackEdge, events: &mut Vec<Event>) {
        if self.back.is_none() {
            self.back = Some(0.0);
            events.push(Event::Back(BackGesture::Started { edge }));
        }
    }
}

fn pen(phase: PenPhase, pos: Pos2, pressure: f32) -> Event {
    Event::Pen {
        phase,
        pos,
        pressure,
        pen: Pen::default(),
    }
}

fn touch(finger: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        id: TouchId { device: 0, finger },
        phase,
        pos,
        force: None,
    }
}
