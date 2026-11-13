use std::collections::BTreeSet;

use beui::{Input, Key, Keystroke, Modifiers};
use smithay::input::keyboard::{keysyms, xkb};

use super::input::InputConfig;
use super::keys::{key, virtual_terminal};

#[derive(Default, Debug, PartialEq)]
pub struct Pressed {
    pub inputs: Vec<Input>,
    pub repeat: Option<Keystroke>,
    pub terminal: Option<i32>,
    pub quit: bool,
}

pub struct Keyboard {
    state: xkb::State,
    modifiers: Modifiers,
    /// The keys held, by evdev code. xkb counts the presses of a key and lets its modifier go
    /// only once as many releases came, so a key pressed twice, or whose release was lost,
    /// would hold its modifier through every later press and release. Each key reaches xkb
    /// once down and once up instead: a press of a held key and a release of a key not held
    /// are dropped, so pressing and releasing a key always lets it go.
    held: BTreeSet<u32>,
}

impl Keyboard {
    pub fn new(config: &InputConfig) -> Option<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            &config.layout,
            &config.variant,
            Some(config.options.clone()).filter(|options| !options.is_empty()),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        let mut keyboard = Self {
            state: xkb::State::new(&keymap),
            modifiers: Modifiers::NONE,
            held: BTreeSet::new(),
        };
        keyboard.set_layout(config.active_layout);
        Some(keyboard)
    }

    pub fn set_layout(&mut self, index: u32) {
        if index >= self.state.get_keymap().num_layouts() {
            return;
        }
        let mods = |kind| self.state.serialize_mods(kind);
        let (depressed, latched, locked) = (
            mods(xkb::STATE_MODS_DEPRESSED),
            mods(xkb::STATE_MODS_LATCHED),
            mods(xkb::STATE_MODS_LOCKED),
        );
        self.state
            .update_mask(depressed, latched, locked, 0, 0, index);
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Takes over the keys `old` holds, as when the keymap changes while keys are down, and
    /// says how the modifiers changed.
    pub fn carry(&mut self, old: &Keyboard) -> Option<Input> {
        for &code in &old.held {
            self.state
                .update_key(xkb::Keycode::new(code + 8), xkb::KeyDirection::Down);
        }
        self.held.clone_from(&old.held);
        self.modifiers = old.modifiers;
        self.settle()
    }

    /// Releases every held key, as when the session leaves its terminal and their releases
    /// will not come.
    pub fn release_all(&mut self) -> Vec<Input> {
        let held: Vec<u32> = self.held.iter().copied().collect();
        held.into_iter()
            .flat_map(|code| self.key(code, false).inputs)
            .collect()
    }

    pub fn key(&mut self, code: u32, pressed: bool) -> Pressed {
        let keycode = xkb::Keycode::new(code + 8);
        let mut out = Pressed::default();
        let changed = match pressed {
            true => self.held.insert(code),
            false => self.held.remove(&code),
        };
        if !changed {
            return out;
        }
        let sym = self.state.key_get_one_sym(keycode).raw();
        let text = self.state.key_get_utf8(keycode);
        self.state.update_key(
            keycode,
            match pressed {
                true => xkb::KeyDirection::Down,
                false => xkb::KeyDirection::Up,
            },
        );
        out.inputs.extend(self.settle());
        let held = self.modifiers;
        if pressed {
            out.terminal = virtual_terminal(sym);
            out.quit = held.ctrl && held.alt && sym == keysyms::KEY_BackSpace;
        }
        let key = key(sym, code).unwrap_or(Key::Unidentified);
        let stroke = Keystroke {
            key: Some(key),
            pressed,
            repeat: false,
            evdev: Some(code),
            text: Some(text),
        };
        if pressed && !key.is_modifier() && out.terminal.is_none() {
            out.repeat = Some(Keystroke {
                repeat: true,
                ..stroke.clone()
            });
        }
        out.inputs.push(Input::Key(stroke));
        out
    }

    fn settle(&mut self) -> Option<Input> {
        let modifiers = Modifiers {
            alt: self.active(xkb::MOD_NAME_ALT),
            ctrl: self.active(xkb::MOD_NAME_CTRL),
            shift: self.active(xkb::MOD_NAME_SHIFT),
            logo: self.active(xkb::MOD_NAME_LOGO),
        };
        if modifiers == self.modifiers {
            return None;
        }
        self.modifiers = modifiers;
        Some(Input::Modifiers(modifiers))
    }

    fn active(&self, name: &str) -> bool {
        self.state
            .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
    }
}

#[cfg(test)]
mod tests;
