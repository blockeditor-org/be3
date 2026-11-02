mod views;

use std::error::Error;

use be_sdk::{EditorHost, ViewHandler, ViewRef};
use beui::WindowId;
use beui::styled::Root;
use beui_core::context::{Context, FrameOutput};
use beui_core::document::Document;
use beui_core::program::Program;

pub use views::{Report, View, Views, anchored_layer, anchored_view, window_view};

pub trait AppRuntime {
    fn launch(
        &self,
        host: &EditorHost,
        keeps_accessibility: bool,
        program: Box<dyn Program>,
    ) -> Result<(), Box<dyn Error>>;

    fn build(&self, root: Box<dyn FnOnce() -> Root>) -> Document;

    fn context(&self) -> Context;

    fn use_bundled_fonts(&self);

    fn runner<'a>(&self, handler: &'a mut dyn ViewHandler) -> Option<&'a mut dyn ViewRunner>;
}

pub trait ViewRunner {
    fn context(&self) -> &Context;

    fn window(&self, region: ViewRef) -> Option<WindowId>;

    fn take_output(&mut self) -> Option<FrameOutput>;
}

// unsafe(ffi)
unsafe extern "Rust" {
    fn __be3_beui_app_runtime() -> &'static dyn AppRuntime;
}

pub fn runtime() -> &'static dyn AppRuntime {
    // unsafe(ffi)
    unsafe { __be3_beui_app_runtime() }
}
