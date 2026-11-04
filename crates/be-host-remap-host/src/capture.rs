//! Catching a chord for a remap editor, before the remapper sees the input.

use std::collections::BTreeSet;

use be_host_remap::names::{BUTTON_FIRST, Modifier};
use be_host_remap::{Input, Source, Wheel};

/// The primary (left) mouse button, which always passes so the editor can be clicked.
const PRIMARY: Source = Source::Button(BUTTON_FIRST + 0x10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caught {
    Source(Source),
    Wheel(Wheel),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Held when it was caught, in the order pressed.
    pub held: Vec<Source>,
    pub trigger: Caught,
}

/// Stands before the remapper. While catching, presses of keys and of buttons other than
/// the primary one, and wheel turns, are kept from the remapper until they make a chord; a
/// key whose press was kept has its release kept too, even after the chord.
#[derive(Default)]
pub struct Capture {
    physical: BTreeSet<Source>,
    kept: BTreeSet<Source>,
    pressed: Vec<Source>,
    catching: bool,
    locked: bool,
    chord: Option<Chord>,
}

impl Capture {
    /// Starts catching anew. Keys held now pass until released, and are not counted.
    pub fn start(&mut self) {
        self.catching = true;
        self.pressed.clear();
        self.chord = None;
    }

    /// Stops catching; keys pressed while it caught stay kept until released.
    pub fn stop(&mut self) {
        self.catching = false;
        self.pressed.clear();
    }

    pub fn catching(&self) -> bool {
        self.catching
    }

    /// Nothing is caught while the screen is locked; locking drops what was pressed so far.
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        if locked {
            self.pressed.clear();
        }
    }

    /// Keys pressed so far in this catch, in the order pressed.
    pub fn held(&self) -> &[Source] {
        &self.pressed
    }

    /// The chord caught, once there is one; catching ends with it.
    pub fn chord(&self) -> Option<&Chord> {
        self.chord.as_ref()
    }

    /// The platform forgot what is held (the session left its terminal).
    pub fn forget_held(&mut self) {
        self.physical.clear();
        self.kept.clear();
        self.pressed.clear();
    }

    /// What goes on to the remapper.
    pub fn input(&mut self, input: Input) -> Vec<Input> {
        match input {
            Input::Press(source) => self.press(source),
            Input::Release(source) => self.release(source),
            Input::Wheel { x, y } => self.wheel(x, y),
        }
    }

    fn active(&self) -> bool {
        self.catching && !self.locked
    }

    fn press(&mut self, source: Source) -> Vec<Input> {
        let repeated = !self.physical.insert(source);
        if self.kept.contains(&source) {
            return Vec::new();
        }
        if !self.active() || source == PRIMARY || repeated {
            return vec![Input::Press(source)];
        }
        if crate::reserved(&self.physical, source) {
            let modifiers: Vec<Source> = self
                .kept
                .iter()
                .copied()
                .filter(|kept| Modifier::of(*kept).is_some())
                .collect();
            let mut out = Vec::new();
            for modifier in modifiers {
                self.kept.remove(&modifier);
                self.pressed.retain(|pressed| *pressed != modifier);
                out.push(Input::Press(modifier));
            }
            out.push(Input::Press(source));
            return out;
        }
        self.kept.insert(source);
        if Modifier::of(source).is_none() && !self.pressed.is_empty() {
            self.caught(Caught::Source(source));
        } else {
            self.pressed.push(source);
        }
        Vec::new()
    }

    fn release(&mut self, source: Source) -> Vec<Input> {
        self.physical.remove(&source);
        if !self.kept.remove(&source) {
            return vec![Input::Release(source)];
        }
        if self.active()
            && self.pressed.contains(&source)
            && let Some(trigger) = self.pressed.pop()
        {
            self.caught(Caught::Source(trigger));
        }
        Vec::new()
    }

    fn wheel(&mut self, x: i32, y: i32) -> Vec<Input> {
        let turn = match (x.signum(), y.signum()) {
            (_, -1) => Wheel::Up,
            (_, 1) => Wheel::Down,
            (-1, _) => Wheel::Left,
            (1, _) => Wheel::Right,
            _ => return vec![Input::Wheel { x, y }],
        };
        if !self.active() {
            return vec![Input::Wheel { x, y }];
        }
        self.caught(Caught::Wheel(turn));
        Vec::new()
    }

    fn caught(&mut self, trigger: Caught) {
        self.chord = Some(Chord {
            held: std::mem::take(&mut self.pressed),
            trigger,
        });
        self.catching = false;
    }
}

#[cfg(test)]
mod tests;
