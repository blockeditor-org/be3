mod arrow;
mod cursors;
mod density;
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
mod outputs;
mod pacing;
mod pointing;
mod plan;
mod problems;
mod recovery;
mod rotation;
mod runner;
mod screen;
mod swapchain;
mod theme;
mod wake;

pub use display::{DisplayConfig, DisplayControl, DisplayMode, DisplayUse, Monitor};
pub use displays::{Display, Displays};
pub use epaper::Epaper;
pub use flips::{Flip, Flips};
pub use gpu::{CursorImage, SoftwareCursor};
pub use input::{
    InputConfig, InputControl, InputDevice, InputFilter, PointerDevice, PointerSettings,
    PointerSpeed, Pointing, RawInput, SharedFilter,
};
pub use problems::Problems;
pub use rotation::Rotation;
pub use runner::{Loop, Session, run};

use beui_runner::{Launch, Platform};

pub struct Drm;

impl Platform for Drm {
    fn name(&self) -> &'static str {
        "drm"
    }

    fn run(self: Box<Self>, launch: Launch) -> beui_runner::Running {
        Box::pin(async move { run(launch) })
    }
}
