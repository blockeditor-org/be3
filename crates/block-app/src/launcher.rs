use std::rc::Rc;

use beui::Event;
use beui::styled::LauncherItem;

use crate::{host, wayland};

const LEFT_SUPER: u32 = 125;
const RIGHT_SUPER: u32 = 126;
const ICON_POINTS: f32 = 32.0;

#[derive(Default)]
pub(crate) struct SuperTap {
    armed: bool,
}

impl SuperTap {
    pub(crate) fn feed(&mut self, events: &[Event]) -> bool {
        let mut tapped = false;
        for event in events {
            match event {
                Event::PhysicalKey {
                    code: LEFT_SUPER | RIGHT_SUPER,
                    pressed,
                } => {
                    tapped |= !pressed && self.armed;
                    self.armed = *pressed;
                }
                Event::PhysicalKey { pressed: true, .. }
                | Event::PointerButton { pressed: true, .. }
                | Event::Scroll(_) => self.armed = false,
                _ => {}
            }
        }
        tapped
    }
}

#[derive(Default)]
pub(crate) struct Launcher {
    open: bool,
    programs: wayland::Programs,
    tap: SuperTap,
}

impl Launcher {
    pub(crate) fn frame(&mut self) {
        self.programs.receive();
        let tapped = host::input(|input| self.tap.feed(&input.events));
        if tapped {
            self.show(!self.open);
        }
    }

    pub(crate) fn show(&mut self, open: bool) {
        if open && !wayland::running() {
            return;
        }
        if open && !self.open {
            let pixels = (ICON_POINTS * host::pixels_per_point()).ceil() as u32;
            self.programs.scan(pixels);
        }
        self.open = open;
    }

    pub(crate) fn launch(&mut self, key: &str) {
        self.open = false;
        self.programs.launch(key);
    }

    pub(crate) fn run(&mut self, line: String) {
        self.open = false;
        wayland::launch(line);
    }

    pub(crate) fn open(&self) -> bool {
        self.open
    }

    pub(crate) fn items(&self) -> Rc<Vec<LauncherItem>> {
        self.programs.items()
    }
}

#[cfg(test)]
mod tests;
