mod arrow;
mod cursors;
mod display;
mod displays;
mod flips;
mod gpu;
mod input;
mod keyboard;
mod keys;
mod layout;
mod modes;
mod output;
mod pacing;
mod problems;
mod recovery;
mod runner;
mod screen;
mod theme;
mod wake;

pub use display::{DisplayConfig, DisplayControl, DisplayMode, Monitor};
pub use flips::{Flip, Flips};
pub use gpu::{CursorImage, SoftwareCursor};
pub use input::{DeviceId, InputConfig, InputControl, PointerConfig, PointerDevice};
pub use problems::Problems;
pub use runner::run;

use beui::{Adapter, Launch};

pub struct Drm;

impl Adapter for Drm {
    fn name(&self) -> &'static str {
        "drm"
    }

    fn run(self: Box<Self>, launch: Launch) -> beui::Running {
        Box::pin(async move { run(launch) })
    }
}
