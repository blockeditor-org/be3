use std::cell::Cell;
use std::ffi::OsStr;
use std::rc::Rc;
use std::time::Duration;

use beui::Waker;
use rustix::ioctl::{Opcode, Setter, Updater, ioctl, opcode};
use smithay::backend::drm::DrmDeviceFd;
use smithay::reexports::drm::Device as _;

const DRIVER: &str = "rockchip-ebc";
const DEFAULT_HINT: &str = "/sys/module/rockchip_ebc/parameters/default_hint";
const HINT_MASK: u8 = 0xf0;
const FOUR_GREYS_AT_ONCE: u8 = 0x10;
const DITHERED_BLACK_AND_WHITE: u8 = 0x40;
const SETTLE_AFTER_INPUT: Duration = Duration::from_millis(500);
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
    touched: Option<Duration>,
}

impl Pace {
    pub(crate) fn touched(&mut self, now: Duration) {
        self.touched = Some(now);
    }

    pub(crate) fn animating(&mut self, animating: bool) {
        self.animating = animating;
    }

    pub(crate) fn settles_at(&self) -> Option<Duration> {
        (self.hurried && !self.animating).then(|| self.settled_at())
    }

    pub(crate) fn step(&mut self, now: Duration) -> Option<Change> {
        let hurried = self.animating || now < self.settled_at();
        if hurried == self.hurried {
            return None;
        }
        self.hurried = hurried;
        Some(match hurried {
            true => Change::Hurry,
            false => Change::Settle,
        })
    }

    fn settled_at(&self) -> Duration {
        self.touched
            .map_or(Duration::ZERO, |touched| touched + SETTLE_AFTER_INPUT)
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
