mod arrow;
mod displays;
mod gpu;
mod keyboard;
mod keys;
mod layout;
mod output;
mod runner;
mod screen;

pub use gpu::{CursorImage, SoftwareCursor};
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
