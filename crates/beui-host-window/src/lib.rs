//! beui's windowed runner: a program in a window of its own, through winit, or
//! Android's activity, with the space and XR layers and beui-host's drive layer
//! stacked as `BEUI_PLATFORM` and `BEUI_LAYERS` ask. Everything else of beui's
//! engine is beui-host's.

use std::error::Error;

use beui_core::program::Program;
use beui_runner::RunOptions;

#[cfg(target_os = "android")]
pub use beui_platform_android::AndroidApp;
#[cfg(not(target_os = "android"))]
pub use beui_platform_winit::winit;

pub use crate::runner::{native_platform, run_with};

mod runner;
mod saved_preferences;

pub fn run(program: impl Program + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::default(), program)
}
