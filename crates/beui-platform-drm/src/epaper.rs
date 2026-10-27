use std::cell::Cell;
use std::ffi::OsStr;
use std::rc::Rc;
use std::time::Duration;

use beui::native::Waker;
use rustix::ioctl::{Opcode, Setter, Updater, ioctl, opcode};
use smithay::backend::drm::DrmDeviceFd;
use smithay::reexports::drm::Device as _;

use crate::wake::{Held, Input};

const DRIVER: &str = "rockchip-ebc";
const DEFAULT_HINT: &str = "/sys/module/rockchip_ebc/parameters/default_hint";
const HINT_MASK: u8 = 0xf0;
const FOUR_GREYS_AT_ONCE: u8 = 0x10;
const DITHERED_BLACK_AND_WHITE: u8 = 0x40;
const QUIET: Duration = Duration::from_millis(100);
const DRM_COMMAND_BASE: u8 = 0x40;

#[repr(C)]
struct GlobalRefresh {
    trigger: bool,
}

#[repr(C)]
struct RectHints {
    set_default_hint: u8,
    default_hint: u8,
    padding: [u8; 2],
    num_rects: u32,
    rect_hints: u64,
}

const GLOBAL_REFRESH: Opcode = opcode::read_write::<GlobalRefresh>(b'd', DRM_COMMAND_BASE);
const RECT_HINTS: Opcode = opcode::write::<RectHints>(b'd', DRM_COMMAND_BASE + 3);

#[derive(Clone)]
pub struct Epaper {
    refreshes: Rc<Cell<bool>>,
    waker: Waker,
}

impl Epaper {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            refreshes: Rc::default(),
            waker,
        }
    }

    pub fn refresh(&self) {
        self.refreshes.set(true);
        self.waker.wake();
    }

    pub(crate) fn take_refresh(&self) -> bool {
        self.refreshes.take()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Hurry,
    Settle,
}

#[derive(Default)]
pub(crate) struct Pace {
    hurried: bool,
    animating: bool,
    held: Vec<Held>,
    active: Option<Duration>,
}

impl Pace {
    pub(crate) fn input(&mut self, input: Input, now: Duration) {
        match input {
            Input::Press(Held::Key(_)) | Input::Release(Held::Key(_)) => {}
            Input::Press(held) => {
                if !self.held.contains(&held) {
                    self.held.push(held);
                }
            }
            Input::Release(held) => {
                self.held.retain(|known| *known != held);
                self.active = Some(now);
            }
            Input::Continue(_) | Input::Other => {}
        }
    }

    pub(crate) fn animating(&mut self, animating: bool) {
        self.animating = animating;
    }

    pub(crate) fn drew(&mut self, now: Duration) {
        if self.hurried {
            self.active = Some(now);
        }
    }

    pub(crate) fn settles_at(&self) -> Option<Duration> {
        (self.hurried && !self.busy()).then(|| self.quiet_at())
    }

    pub(crate) fn step(&mut self, now: Duration) -> Option<Change> {
        match self.hurried {
            false if self.busy() => {
                self.hurried = true;
                self.active = Some(now);
                Some(Change::Hurry)
            }
            true if !self.busy() && now >= self.quiet_at() => {
                self.hurried = false;
                Some(Change::Settle)
            }
            _ => None,
        }
    }

    fn busy(&self) -> bool {
        self.animating || !self.held.is_empty()
    }

    fn quiet_at(&self) -> Duration {
        self.active.map_or(Duration::ZERO, |active| active + QUIET)
    }
}

pub(crate) struct Panel {
    drm: DrmDeviceFd,
    found: Option<u8>,
    pub(crate) pace: Pace,
}

impl Panel {
    pub(crate) fn find(drm: &DrmDeviceFd) -> Option<Self> {
        let driver = drm.get_driver().ok()?;
        if driver.name() != OsStr::new(DRIVER) {
            return None;
        }
        let found = std::fs::read_to_string(DEFAULT_HINT)
            .ok()
            .and_then(|hint| hint.trim().parse::<u8>().ok())
            .map(|hint| hint & HINT_MASK);
        Some(Self {
            drm: drm.clone(),
            found,
            pace: Pace::default(),
        })
    }

    pub(crate) fn take_over(&mut self) {
        self.pace = Pace::default();
        self.hint(FOUR_GREYS_AT_ONCE);
    }

    pub(crate) fn change(&self, change: Change) {
        self.hint(match change {
            Change::Hurry => DITHERED_BLACK_AND_WHITE,
            Change::Settle => FOUR_GREYS_AT_ONCE,
        });
    }

    fn hint(&self, hint: u8) {
        if let Err(error) = self.set_default_hint(hint) {
            eprintln!("beui: the e-paper display's refresh mode could not be set: {error}");
        }
    }

    pub(crate) fn refresh(&self) {
        let mut refresh = GlobalRefresh { trigger: true };
        // unsafe(ffi)
        let refreshed = unsafe {
            ioctl(
                &self.drm,
                Updater::<GLOBAL_REFRESH, GlobalRefresh>::new(&mut refresh),
            )
        };
        if let Err(error) = refreshed {
            eprintln!("beui: the e-paper display could not be refreshed: {error}");
        }
    }

    fn set_default_hint(&self, hint: u8) -> rustix::io::Result<()> {
        let hints = RectHints {
            set_default_hint: 1,
            default_hint: hint,
            padding: [0; 2],
            num_rects: 0,
            rect_hints: 0,
        };
        // unsafe(ffi)
        unsafe { ioctl(&self.drm, Setter::<RECT_HINTS, RectHints>::new(hints)) }
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        if let Some(hint) = self.found {
            let _ = self.set_default_hint(hint);
        }
    }
}

#[cfg(test)]
mod tests;
