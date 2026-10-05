use beui::{Event, Modifiers};
use smithay::input::keyboard::{keysyms, xkb};

use super::keys::{key, virtual_terminal};

#[derive(Default, Debug, PartialEq)]
pub struct Pressed {
    pub events: Vec<Event>,
    pub repeat: Option<Vec<Event>>,
    pub terminal: Option<i32>,
    pub quit: bool,
}

pub struct Keyboard {
    state: xkb::State,
    modifiers: Modifiers,
}

impl Keyboard {
    pub fn new() -> Option<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        Some(Self {
            state: xkb::State::new(&keymap),
            modifiers: Modifiers::NONE,
        })
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn key(&mut self, code: u32, pressed: bool) -> Pressed {
        let keycode = xkb::Keycode::new(code + 8);
        let mut out = Pressed::default();
        out.events.push(Event::PhysicalKey { code, pressed });
        let sym = self.state.key_get_one_sym(keycode).raw();
        let text = self.state.key_get_utf8(keycode);
        self.state.update_key(
            keycode,
            match pressed {
                true => xkb::KeyDirection::Down,
                false => xkb::KeyDirection::Up,
            },
        );
        let modifiers = Modifiers {
            alt: self.active(xkb::MOD_NAME_ALT),
            ctrl: self.active(xkb::MOD_NAME_CTRL) || self.active(xkb::MOD_NAME_LOGO),
            shift: self.active(xkb::MOD_NAME_SHIFT),
        };
        if modifiers != self.modifiers {
            self.modifiers = modifiers;
            out.events.push(Event::Modifiers(modifiers));
        }
        let held = self.modifiers;
        if pressed {
            out.terminal = virtual_terminal(sym);
            out.quit = held.ctrl && held.alt && sym == keysyms::KEY_BackSpace;
        }
        let mut repeated = Vec::new();
        if let Some(key) = key(sym) {
            out.events.push(Event::Key {
                key,
                pressed,
                repeat: false,
                modifiers: held,
            });
            repeated.push(Event::Key {
                key,
                pressed: true,
                repeat: true,
                modifiers: held,
            });
        }
        if pressed
            && !held.ctrl
            && !held.alt
            && !text.is_empty()
            && !text.chars().any(char::is_control)
        {
            out.events.push(Event::Text(text.clone()));
            repeated.push(Event::Text(text));
        }
        if pressed && !repeated.is_empty() && out.terminal.is_none() {
            out.repeat = Some(repeated);
        }
        out
    }

    fn active(&self, name: &str) -> bool {
        self.state
            .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
    }
}

#[cfg(test)]
mod tests;
