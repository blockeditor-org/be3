use std::collections::HashMap;

use beui::Key;
use smithay::backend::input::KeyState;
use smithay::input::keyboard::{KeyboardTarget, Keycode, ModifiersState, xkb};
use smithay::utils::SERIAL_COUNTER;

use crate::state::State;

const EVDEV_OFFSET: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    pub code: u32,
    pub set: u32,
    pub levers: u32,
    pub unlocks: u32,
}

pub(crate) fn find(keymap: &xkb::Keymap, layout: u32, wanted: char, shifts: bool) -> Option<Placed> {
    let index = |name: &str| match keymap.mod_get_index(name) {
        xkb::MOD_INVALID => 0,
        index => 1 << index,
    };
    let shift = if shifts { index(xkb::MOD_NAME_SHIFT) } else { 0 };
    let level3 = index(xkb::MOD_NAME_ISO_LEVEL3_SHIFT);
    let level5 = index(xkb::MOD_NAME_MOD3);
    let levers = shift | level3 | level5;
    let caps = if shifts { index(xkb::MOD_NAME_CAPS) } else { 0 };
    let mut tried = Vec::new();
    for set in [
        0,
        shift,
        level3,
        shift | level3,
        level5,
        shift | level5,
        level3 | level5,
        shift | level3 | level5,
    ] {
        if tried.contains(&set) {
            continue;
        }
        tried.push(set);
        let mut state = xkb::State::new(keymap);
        state.update_mask(set, 0, 0, 0, 0, layout);
        let min = keymap.min_keycode().raw();
        let max = keymap.max_keycode().raw();
        for raw in min.max(EVDEV_OFFSET)..=max {
            if state.key_get_utf32(Keycode::new(raw)) == u32::from(wanted) {
                return Some(Placed {
                    code: raw - EVDEV_OFFSET,
                    set,
                    levers,
                    unlocks: levers | caps,
                });
            }
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Held {
    Placed(u32),
    Keyed(u32),
}

#[derive(Default)]
pub(crate) struct Placeless {
    held: HashMap<Key, Held>,
}

impl State {
    pub fn placeless_key(&mut self, key: Key, text: Option<&str>, pressed: bool) -> bool {
        if !pressed {
            match self.placeless.held.remove(&key) {
                Some(Held::Placed(code)) => self.forward(code, false, None),
                Some(Held::Keyed(code)) => self.key(code, false),
                None => return false,
            }
            return true;
        }
        let single = text.and_then(|text| {
            let mut chars = text.chars();
            chars.next().filter(|_| chars.next().is_none())
        });
        let wanted = match text {
            Some(_) => single,
            None => beui::input::typing::character(key),
        };
        if let Some(placed) = wanted.and_then(|wanted| self.layout_place(wanted, text.is_some())) {
            let held = self.placeless.held.insert(key, Held::Placed(placed.code));
            self.release_held(held);
            self.forward(placed.code, true, Some(placed));
            return true;
        }
        if text.is_some() {
            return false;
        }
        let Some(code) = key.evdev() else {
            return false;
        };
        let held = self.placeless.held.insert(key, Held::Keyed(code));
        self.release_held(held);
        self.key(code, true);
        true
    }

    fn release_held(&mut self, held: Option<Held>) {
        match held {
            Some(Held::Placed(code)) => self.forward(code, false, None),
            Some(Held::Keyed(code)) => self.key(code, false),
            None => {}
        }
    }

    fn layout_place(&mut self, wanted: char, shifts: bool) -> Option<Placed> {
        let keyboard = self.keyboard();
        keyboard.with_xkb_state(self, |context| {
            let xkb = context.xkb().lock().unwrap();
            let layout = xkb.active_layout().0;
            // unsafe(ffi)
            let keymap = unsafe { xkb.keymap() };
            find(keymap, layout, wanted, shifts)
        })
    }

    fn forward(&mut self, code: u32, pressed: bool, placed: Option<Placed>) {
        let keyboard = self.keyboard();
        let time = self.time();
        let current = keyboard.modifier_state();
        let leveled = placed
            .map(|placed| level(current, placed))
            .filter(|leveled| leveled.serialized != current.serialized);
        let focus = keyboard.current_focus();
        let seat = self.seat.clone();
        if let (Some(leveled), Some(focus)) = (leveled, &focus) {
            focus.modifiers(&seat, self, leveled, SERIAL_COUNTER.next_serial());
        }
        let state = match pressed {
            true => KeyState::Pressed,
            false => KeyState::Released,
        };
        keyboard.input_forward(
            self,
            Keycode::new(code + EVDEV_OFFSET),
            state,
            SERIAL_COUNTER.next_serial(),
            time,
            false,
        );
        if let (Some(_), Some(focus)) = (leveled, &focus) {
            focus.modifiers(&seat, self, current, SERIAL_COUNTER.next_serial());
        }
    }
}

fn level(current: ModifiersState, placed: Placed) -> ModifiersState {
    let mut leveled = current;
    let serialized = &mut leveled.serialized;
    serialized.depressed = (serialized.depressed & !placed.levers) | placed.set;
    serialized.latched &= !placed.levers;
    serialized.locked &= !placed.unlocks;
    leveled
}
