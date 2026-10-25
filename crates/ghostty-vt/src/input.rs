use std::ffi::c_int;
use std::ptr;

use crate::sys;
use crate::terminal::{Error, Terminal, check};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key(c_int);

impl Key {
    pub const UNIDENTIFIED: Self = Self(0);
    pub const BACKQUOTE: Self = Self(1);
    pub const BACKSLASH: Self = Self(2);
    pub const BRACKET_LEFT: Self = Self(3);
    pub const BRACKET_RIGHT: Self = Self(4);
    pub const COMMA: Self = Self(5);
    pub const DIGIT_0: Self = Self(6);
    pub const DIGIT_1: Self = Self(7);
    pub const DIGIT_2: Self = Self(8);
    pub const DIGIT_3: Self = Self(9);
    pub const DIGIT_4: Self = Self(10);
    pub const DIGIT_5: Self = Self(11);
    pub const DIGIT_6: Self = Self(12);
    pub const DIGIT_7: Self = Self(13);
    pub const DIGIT_8: Self = Self(14);
    pub const DIGIT_9: Self = Self(15);
    pub const EQUAL: Self = Self(16);
    pub const INTL_BACKSLASH: Self = Self(17);
    pub const INTL_RO: Self = Self(18);
    pub const INTL_YEN: Self = Self(19);
    pub const A: Self = Self(20);
    pub const B: Self = Self(21);
    pub const C: Self = Self(22);
    pub const D: Self = Self(23);
    pub const E: Self = Self(24);
    pub const F: Self = Self(25);
    pub const G: Self = Self(26);
    pub const H: Self = Self(27);
    pub const I: Self = Self(28);
    pub const J: Self = Self(29);
    pub const K: Self = Self(30);
    pub const L: Self = Self(31);
    pub const M: Self = Self(32);
    pub const N: Self = Self(33);
    pub const O: Self = Self(34);
    pub const P: Self = Self(35);
    pub const Q: Self = Self(36);
    pub const R: Self = Self(37);
    pub const S: Self = Self(38);
    pub const T: Self = Self(39);
    pub const U: Self = Self(40);
    pub const V: Self = Self(41);
    pub const W: Self = Self(42);
    pub const X: Self = Self(43);
    pub const Y: Self = Self(44);
    pub const Z: Self = Self(45);
    pub const MINUS: Self = Self(46);
    pub const PERIOD: Self = Self(47);
    pub const QUOTE: Self = Self(48);
    pub const SEMICOLON: Self = Self(49);
    pub const SLASH: Self = Self(50);
    pub const ALT_LEFT: Self = Self(51);
    pub const ALT_RIGHT: Self = Self(52);
    pub const BACKSPACE: Self = Self(53);
    pub const CAPS_LOCK: Self = Self(54);
    pub const CONTEXT_MENU: Self = Self(55);
    pub const CONTROL_LEFT: Self = Self(56);
    pub const CONTROL_RIGHT: Self = Self(57);
    pub const ENTER: Self = Self(58);
    pub const META_LEFT: Self = Self(59);
    pub const META_RIGHT: Self = Self(60);
    pub const SHIFT_LEFT: Self = Self(61);
    pub const SHIFT_RIGHT: Self = Self(62);
    pub const SPACE: Self = Self(63);
    pub const TAB: Self = Self(64);
    pub const CONVERT: Self = Self(65);
    pub const KANA_MODE: Self = Self(66);
    pub const NON_CONVERT: Self = Self(67);
    pub const DELETE: Self = Self(68);
    pub const END: Self = Self(69);
    pub const HELP: Self = Self(70);
    pub const HOME: Self = Self(71);
    pub const INSERT: Self = Self(72);
    pub const PAGE_DOWN: Self = Self(73);
    pub const PAGE_UP: Self = Self(74);
    pub const ARROW_DOWN: Self = Self(75);
    pub const ARROW_LEFT: Self = Self(76);
    pub const ARROW_RIGHT: Self = Self(77);
    pub const ARROW_UP: Self = Self(78);
    pub const ESCAPE: Self = Self(120);
    pub const F1: Self = Self(121);
    pub const F2: Self = Self(122);
    pub const F3: Self = Self(123);
    pub const F4: Self = Self(124);
    pub const F5: Self = Self(125);
    pub const F6: Self = Self(126);
    pub const F7: Self = Self(127);
    pub const F8: Self = Self(128);
    pub const F9: Self = Self(129);
    pub const F10: Self = Self(130);
    pub const F11: Self = Self(131);
    pub const F12: Self = Self(132);
    pub const F13: Self = Self(133);
    pub const F14: Self = Self(134);
    pub const F15: Self = Self(135);
    pub const F16: Self = Self(136);
    pub const F17: Self = Self(137);
    pub const F18: Self = Self(138);
    pub const F19: Self = Self(139);
    pub const F20: Self = Self(140);
    pub const F21: Self = Self(141);
    pub const F22: Self = Self(142);
    pub const F23: Self = Self(143);
    pub const F24: Self = Self(144);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Mods {
    fn bits(self) -> u16 {
        let mut bits = 0;
        for (on, bit) in [
            (self.shift, sys::MODS_SHIFT),
            (self.ctrl, sys::MODS_CTRL),
            (self.alt, sys::MODS_ALT),
            (self.logo, sys::MODS_SUPER),
        ] {
            if on {
                bits |= bit;
            }
        }
        bits
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInput<'a> {
    pub key: Key,
    pub mods: Mods,
    pub text: &'a str,
    pub unshifted: Option<char>,
    pub pressed: bool,
}

pub struct KeyEncoder {
    encoder: sys::KeyEncoder,
    event: sys::KeyEvent,
}

// unsafe(ffi)
impl KeyEncoder {
    pub fn new() -> Result<Self, Error> {
        let mut encoder: sys::KeyEncoder = ptr::null_mut();
        check(unsafe { sys::ghostty_key_encoder_new(ptr::null(), &mut encoder) })?;
        let mut event: sys::KeyEvent = ptr::null_mut();
        if let Err(error) = check(unsafe { sys::ghostty_key_event_new(ptr::null(), &mut event) }) {
            unsafe { sys::ghostty_key_encoder_free(encoder) };
            return Err(error);
        }
        Ok(Self { encoder, event })
    }

    pub fn encode(&mut self, terminal: &Terminal, input: &KeyInput<'_>) -> Vec<u8> {
        let event = self.event;
        unsafe {
            sys::ghostty_key_encoder_setopt_from_terminal(self.encoder, terminal.handle());
            sys::ghostty_key_event_set_action(
                event,
                match input.pressed {
                    true => sys::KEY_ACTION_PRESS,
                    false => sys::KEY_ACTION_RELEASE,
                },
            );
            sys::ghostty_key_event_set_key(event, input.key.0);
            sys::ghostty_key_event_set_mods(event, input.mods.bits());
            let consumed = match input.mods.shift && !input.text.is_empty() {
                true => sys::MODS_SHIFT,
                false => 0,
            };
            sys::ghostty_key_event_set_consumed_mods(event, consumed);
            sys::ghostty_key_event_set_utf8(event, input.text.as_ptr().cast(), input.text.len());
            sys::ghostty_key_event_set_unshifted_codepoint(
                event,
                input.unshifted.map_or(0, u32::from),
            );
        }
        let mut buffer = [0u8; 128];
        let mut length = 0usize;
        let result = unsafe {
            sys::ghostty_key_encoder_encode(
                self.encoder,
                event,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut length,
            )
        };
        match result {
            sys::SUCCESS => buffer[..length.min(buffer.len())].to_vec(),
            sys::OUT_OF_SPACE => {
                let mut buffer = vec![0u8; length];
                let mut written = 0usize;
                let result = unsafe {
                    sys::ghostty_key_encoder_encode(
                        self.encoder,
                        event,
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                        &mut written,
                    )
                };
                buffer.truncate(written);
                if result == sys::SUCCESS {
                    buffer
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
}

// unsafe(ffi)
impl Drop for KeyEncoder {
    fn drop(&mut self) {
        unsafe {
            sys::ghostty_key_event_free(self.event);
            sys::ghostty_key_encoder_free(self.encoder);
        }
    }
}

pub struct MouseEncoder {
    encoder: sys::MouseEncoder,
    event: sys::MouseEvent,
}

const CELL: u32 = 8;

// unsafe(ffi)
impl MouseEncoder {
    pub fn new() -> Result<Self, Error> {
        let mut encoder: sys::MouseEncoder = ptr::null_mut();
        check(unsafe { sys::ghostty_mouse_encoder_new(ptr::null(), &mut encoder) })?;
        let mut event: sys::MouseEvent = ptr::null_mut();
        if let Err(error) = check(unsafe { sys::ghostty_mouse_event_new(ptr::null(), &mut event) })
        {
            unsafe { sys::ghostty_mouse_encoder_free(encoder) };
            return Err(error);
        }
        Ok(Self { encoder, event })
    }

    pub fn wheel(
        &mut self,
        terminal: &Terminal,
        up: bool,
        col: u16,
        row: u16,
        mods: Mods,
    ) -> Vec<u8> {
        let (cols, rows) = terminal.size();
        let size = sys::MouseEncoderSize {
            size: size_of::<sys::MouseEncoderSize>(),
            screen_width: u32::from(cols) * CELL,
            screen_height: u32::from(rows) * CELL,
            cell_width: CELL,
            cell_height: CELL,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        };
        let event = self.event;
        unsafe {
            sys::ghostty_mouse_encoder_setopt_from_terminal(self.encoder, terminal.handle());
            sys::ghostty_mouse_encoder_setopt(
                self.encoder,
                sys::MOUSE_ENCODER_OPT_SIZE,
                ptr::from_ref(&size).cast(),
            );
            sys::ghostty_mouse_event_set_action(event, sys::MOUSE_ACTION_PRESS);
            sys::ghostty_mouse_event_set_button(
                event,
                match up {
                    true => sys::MOUSE_BUTTON_FOUR,
                    false => sys::MOUSE_BUTTON_FIVE,
                },
            );
            sys::ghostty_mouse_event_set_mods(event, mods.bits());
            sys::ghostty_mouse_event_set_position(
                event,
                sys::MousePosition {
                    x: (f32::from(col.min(cols.saturating_sub(1))) + 0.5) * CELL as f32,
                    y: (f32::from(row.min(rows.saturating_sub(1))) + 0.5) * CELL as f32,
                },
            );
        }
        let mut buffer = [0u8; 64];
        let mut length = 0usize;
        let result = unsafe {
            sys::ghostty_mouse_encoder_encode(
                self.encoder,
                event,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut length,
            )
        };
        match result {
            sys::SUCCESS => buffer[..length.min(buffer.len())].to_vec(),
            _ => Vec::new(),
        }
    }
}

// unsafe(ffi)
impl Drop for MouseEncoder {
    fn drop(&mut self) {
        unsafe {
            sys::ghostty_mouse_event_free(self.event);
            sys::ghostty_mouse_encoder_free(self.encoder);
        }
    }
}
