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
            ctrl: self.active(xkb::MOD_NAME_CTRL),
            shift: self.active(xkb::MOD_NAME_SHIFT),
            logo: self.active(xkb::MOD_NAME_LOGO),
        };
        if modifiers != self.modifiers {
            self.modifiers = modifiers;
            out.inputs.push(Input::Modifiers(modifiers));
        }
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

    fn active(&self, name: &str) -> bool {
        self.state
            .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
    }
}

#[cfg(test)]
mod tests;
