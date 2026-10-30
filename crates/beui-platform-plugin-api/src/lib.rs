mod views;

use std::error::Error;

use beui::styled::Root;
use beui_core::app::App;
use beui_core::context::{Context, FrameOutput};
use beui_core::document::Document;
use beui_core::window::WindowId;
use block_editor_plugin::{EditorHost, ViewHandler, ViewRef};

pub use views::{Report, View, Views, anchored_layer, anchored_view, window_view};

pub trait PluginRuntime {
    fn launch(
        &self,
        host: &EditorHost,
        keeps_accessibility: bool,
        app: Box<dyn App>,
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
    fn __be3_beui_plugin_runtime() -> &'static dyn PluginRuntime;
}

pub fn runtime() -> &'static dyn PluginRuntime {
    // unsafe(ffi)
    unsafe { __be3_beui_plugin_runtime() }
}
