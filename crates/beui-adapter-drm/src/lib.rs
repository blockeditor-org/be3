mod arrow;
mod displays;
mod gpu;
mod input;
mod keyboard;
mod keys;
mod layout;
mod output;
mod problems;
mod runner;
mod screen;

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
