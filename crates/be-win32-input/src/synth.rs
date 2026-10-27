use beui_types::input::{Keystroke, PointerButton, ScrollDelta};
use beui_types::input::{SCROLL_LINE, scrolled};
use beui_types::{Pos2, Vec2};

use crate::mapping::{PixelPoint, PixelRect, absolute};
use crate::scancode::{scan_of_evdev, virtual_key_of_evdev};

pub mod flags {
    pub const MOUSE_MOVE: u32 = 0x0001;
    pub const MOUSE_LEFT_DOWN: u32 = 0x0002;
    pub const MOUSE_LEFT_UP: u32 = 0x0004;
    pub const MOUSE_RIGHT_DOWN: u32 = 0x0008;
    pub const MOUSE_RIGHT_UP: u32 = 0x0010;
    pub const MOUSE_MIDDLE_DOWN: u32 = 0x0020;
    pub const MOUSE_MIDDLE_UP: u32 = 0x0040;
    pub const MOUSE_X_DOWN: u32 = 0x0080;
    pub const MOUSE_X_UP: u32 = 0x0100;
    pub const MOUSE_WHEEL: u32 = 0x0800;
    pub const MOUSE_HORIZONTAL_WHEEL: u32 = 0x1000;
    pub const MOUSE_VIRTUAL_DESK: u32 = 0x4000;
    pub const MOUSE_ABSOLUTE: u32 = 0x8000;
    pub const XBUTTON1: i32 = 0x0001;
    pub const XBUTTON2: i32 = 0x0002;

    pub const KEY_EXTENDED: u32 = 0x0001;
    pub const KEY_UP: u32 = 0x0002;
    pub const KEY_UNICODE: u32 = 0x0004;
    pub const KEY_SCANCODE: u32 = 0x0008;
}

pub const WHEEL_DELTA: f32 = 120.0;

pub const INJECTED: usize = 0x6265_3349;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Synth {
    Mouse {
        x: i32,
        y: i32,
        data: i32,
        flags: u32,
    },
    Key {
        vk: u16,
        scan: u16,
        flags: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerAction {
    Move,
    Press(PointerButton),
    Release(PointerButton),
    Scroll(ScrollDelta),
}

pub fn button_flags(button: PointerButton, pressed: bool) -> Option<(u32, i32)> {
    use flags::*;
    let (down, up, data) = match button {
        PointerButton::Primary => (MOUSE_LEFT_DOWN, MOUSE_LEFT_UP, 0),
        PointerButton::Secondary => (MOUSE_RIGHT_DOWN, MOUSE_RIGHT_UP, 0),
        PointerButton::Middle => (MOUSE_MIDDLE_DOWN, MOUSE_MIDDLE_UP, 0),
        PointerButton::Back => (MOUSE_X_DOWN, MOUSE_X_UP, XBUTTON1),
        PointerButton::Forward => (MOUSE_X_DOWN, MOUSE_X_UP, XBUTTON2),
        PointerButton::Other(_) => return None,
    };
    Some((if pressed { down } else { up }, data))
}

pub fn wheel_units(delta: ScrollDelta) -> (i32, i32) {
    let lines = scrolled(delta, 1.0) / SCROLL_LINE;
    (
        (-lines.x * WHEEL_DELTA).round() as i32,
        (lines.y * WHEEL_DELTA).round() as i32,
    )
}

pub fn pointer(desktop: PixelRect, at: PixelPoint, action: PointerAction) -> Vec<Synth> {
    use flags::*;
    let to = absolute(desktop, at);
    let moved = MOUSE_MOVE | MOUSE_ABSOLUTE | MOUSE_VIRTUAL_DESK;
    let mouse = |data, flags| Synth::Mouse {
        x: to.x,
        y: to.y,
        data,
        flags,
    };
    let button = |button, pressed| match button_flags(button, pressed) {
        Some((flags, data)) => vec![mouse(data, moved | flags)],
        None => vec![mouse(0, moved)],
    };
    match action {
        PointerAction::Move => vec![mouse(0, moved)],
        PointerAction::Press(pressed) => button(pressed, true),
        PointerAction::Release(released) => button(released, false),
        PointerAction::Scroll(delta) => {
            let (horizontal, vertical) = wheel_units(delta);
            let mut synths = vec![mouse(0, moved)];
            if vertical != 0 {
                synths.push(mouse(vertical, MOUSE_WHEEL));
            }
            if horizontal != 0 {
                synths.push(mouse(horizontal, MOUSE_HORIZONTAL_WHEEL));
            }
            synths
        }
    }
}

pub fn key(evdev: u32, pressed: bool) -> Option<Synth> {
    use flags::*;
    let up = match pressed {
        true => 0,
        false => KEY_UP,
    };
    if let Some(code) = scan_of_evdev(evdev) {
        let extended = match code.extended {
            true => KEY_EXTENDED,
            false => 0,
        };
        return Some(Synth::Key {
            vk: 0,
            scan: code.scan,
            flags: KEY_SCANCODE | extended | up,
        });
    }
    virtual_key_of_evdev(evdev).map(|vk| Synth::Key {
        vk,
        scan: 0,
        flags: up,
    })
}

pub fn text(text: &str) -> Vec<Synth> {
    let mut synths = Vec::new();
    for character in text.chars() {
        let named = match character {
            '\n' | '\r' => key(28, true).zip(key(28, false)),
            '\t' => key(15, true).zip(key(15, false)),
            _ => None,
        };
        if let Some((down, up)) = named {
            synths.extend([down, up]);
            continue;
        }
        let mut units = [0; 2];
        for unit in character.encode_utf16(&mut units) {
            for up in [0, flags::KEY_UP] {
                synths.push(Synth::Key {
                    vk: 0,
                    scan: *unit,
                    flags: flags::KEY_UNICODE | up,
                });
            }
        }
    }
    synths
}

pub fn keystroke(stroke: &Keystroke) -> Vec<Synth> {
    let evdev = stroke
        .evdev
        .or_else(|| stroke.key.and_then(|key| key.evdev()));
    if let Some(synth) = evdev.and_then(|evdev| key(evdev, stroke.pressed)) {
        return vec![synth];
    }
    match (stroke.pressed, &stroke.text) {
        (true, Some(typed)) => text(typed),
        _ => Vec::new(),
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Pointer { at: Pos2, action: PointerAction },
    Key(Keystroke),
    Text(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Planned {
    pub cursor: Option<PixelPoint>,
    pub synths: Vec<Synth>,
}

pub fn plan(desktop: PixelRect, frame: PixelRect, size: Vec2, event: &Event) -> Planned {
    match event {
        Event::Pointer { at, action } => {
            let point = frame.point_at(size, *at);
            Planned {
                cursor: Some(point),
                synths: pointer(desktop, point, *action),
            }
        }
        Event::Key(stroke) => Planned {
            cursor: None,
            synths: keystroke(stroke),
        },
        Event::Text(typed) => Planned {
            cursor: None,
            synths: text(typed),
        },
    }
}

#[cfg(test)]
mod tests;
