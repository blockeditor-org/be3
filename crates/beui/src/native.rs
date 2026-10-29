#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
use std::error::Error;

pub use beui_core::accessibility;
pub use beui_core::app::accessibility_dump;
pub use beui_core::app::{App, FrameTimes, Lifecycle, NativeWindow, Setup, Waker};
pub use beui_core::base::embed::{EmbedPlacement, EmbedSlot};
pub use beui_core::clipboard::{PasteAnswer, PasteRequest};
pub use beui_core::context::{
    Context, FrameOutput, InputSimulation, Moved, RendererInfo, WindowOutput,
};
pub use beui_core::current::{
    enter, last_pointer, try_with_document, with_document, with_reactive_scope,
};
pub use beui_core::damage::Region;
pub use beui_core::document::Document;
pub use beui_core::document::{
    OverRepaint, Tools, detect_over_repaint, fail_unsettled_layout, take_over_repaints,
    verify_paint,
};
pub use beui_core::draw::{Quad, Quads, Turn, quads};
pub use beui_core::drawing::Drawing;
pub use beui_core::drawn_cursor;
pub use beui_core::file_picker::{FilePickAnswer, FilePickId, FilePickRequest};
pub use beui_core::file_saver::{FileSaveAnswer, FileSaveId, FileSaveRequest};
pub use beui_core::filter::{ColorVision, Filter, MAX_BLUR};
pub use beui_core::focus::focus_within;
pub use beui_core::font::{
    FontBackend, Galley, GalleyLine, Glyph, GlyphId, GlyphImage, Shaping, Wraps, line_height,
};
pub use beui_core::input::InputState;
pub use beui_core::node::{ClickHandler, Handler, NodeOf};
pub use beui_core::overflow;
pub use beui_core::painter::{Painter as NativePainter, Shape};
pub use beui_core::painting;
pub use beui_core::path::PathPaint;
pub use beui_core::performance::{
    FramePerformance, FrameWork, PerformanceSnapshot, PerformanceTimings,
};
pub use beui_core::rich;
pub use beui_core::runner::environment;
pub use beui_core::runner::{
    Incoming, Launch, Layer, LayerFrame, LayerView, Platform, PlatformServices, RunOptions,
    Running, Shown, Units, panic_message,
};
pub use beui_core::touch_cursor;
pub use beui_core::window::{Resize, WindowHosting, WindowSize};
pub use beui_draw::{Draw, DrawAt, drawing, painted_drawing};
#[cfg(not(target_arch = "wasm32"))]
pub use beui_font_freetype::system::SystemFonts;
pub use beui_font_freetype::{
    FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts, ICONS_FONT,
};
pub use beui_inspector::install as install_inspector;
pub use beui_layer_drive as drive;
#[cfg(all(feature = "window", target_os = "android"))]
pub use beui_platform_android::AndroidApp;
#[cfg(all(
    feature = "window",
    not(any(target_os = "android", target_arch = "wasm32"))
))]
pub use beui_platform_headless::Headless;
pub use beui_platform_headless::Simulation;
pub use beui_platform_headless::testing;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use beui_platform_web::accessibility_tree;
#[cfg(all(feature = "window", not(target_os = "android")))]
pub use beui_platform_winit::winit;
#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
pub use beui_renderer_wgpu::present::read_texture;
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::present::{GpuSetup, OpenDevice, OpenGpu, open_vulkan_device};
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::{Renderer, Repaint, Repainting, renderer_info, wgpu_color};
pub use beui_view::native_api::{core_painter, draw_gpu, wrap_painter};
pub use embed::Embed;

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use crate::runner::{native_platform, run_on, run_with, run_with_device};
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use crate::web::{run_web, web_platform};

mod embed;

pub fn build(f: impl FnOnce() -> beui_view::primitives::Root) -> Document {
    let mut document = beui_view::native_api::build(f);
    beui_inspector::install(&mut document);
    crate::keyboard_host::install_keyboard(&mut document);
    document
}

pub fn context() -> Context {
    Context::new(FreetypeFonts::default())
}

pub fn system_fonts() -> FontLibrary {
    let fonts = FontLibrary::bundled();
    #[cfg(not(target_arch = "wasm32"))]
    let fonts = fonts.with_fallback(beui_font_freetype::system::fallback());
    fonts
}

pub fn system_context() -> Context {
    Context::new(FreetypeFonts::new(system_fonts()))
}

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub fn run(app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::default(), app)
}
