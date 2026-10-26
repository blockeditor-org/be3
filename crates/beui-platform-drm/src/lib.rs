mod arrow;
mod cursors;
mod display;
mod displays;
mod epaper;
mod escape;
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
mod rotation;
mod runner;
mod screen;
mod swapchain;
mod theme;
mod wake;

pub use display::{DisplayConfig, DisplayControl, DisplayMode, Monitor};
pub use epaper::Epaper;
pub use flips::{Flip, Flips};
pub use gpu::{CursorImage, SoftwareCursor};
pub use input::{
    InputConfig, InputControl, InputDevice, InputFilter, PointerDevice, PointerSettings,
    PointerSpeed, RawInput, SharedFilter,
};
pub use problems::Problems;
pub use rotation::Rotation;
pub use runner::run;

use beui::{Launch, Platform};

pub struct Drm;

impl Platform for Drm {
    fn name(&self) -> &'static str {
        "drm"
    }

    fn run(self: Box<Self>, launch: Launch) -> beui::Running {
        Box::pin(async move { run(launch) })
    }
}
