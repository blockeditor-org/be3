mod views;

use std::error::Error;

use beui_core::app::App;
use beui_core::context::{Context, FrameOutput};
use beui_core::document::Document;
use beui_core::runner::RunOptions;
use beui_core::window::WindowId;
use beui_view::primitives::Root;
use block_editor_plugin::{EditorHost, RegionId, Regions};

pub use views::{Report, View, Views};

pub trait PluginRuntime {
    fn launch(
        &self,
        host: &EditorHost,
        options: RunOptions,
        app: Box<dyn App>,
    ) -> Result<(), Box<dyn Error>>;

    fn build(&self, root: Box<dyn FnOnce() -> Root>) -> Document;

    fn context(&self) -> Context;

    fn use_bundled_fonts(&self);

    fn runner<'a>(&self, handler: &'a mut dyn Regions) -> Option<&'a mut dyn RegionsRunner>;
}

pub trait RegionsRunner {
    fn context(&self) -> &Context;

    fn window(&self, region: RegionId) -> Option<WindowId>;

    fn take_output(&mut self) -> Option<FrameOutput>;
}

// unsafe(ffi)
unsafe extern "Rust" {
    fn __be3_beui_plugin_runtime() -> &'static dyn PluginRuntime;
}

pub fn runtime() -> &'static dyn PluginRuntime {
    // unsafe(ffi)
    unsafe { __be3_beui_plugin_runtime() }
}
