#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
use std::error::Error;

pub use beui::native::*;
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
#[cfg(not(target_arch = "wasm32"))]
pub use beui_renderer_wgpu::present::read_texture;
pub use beui_renderer_wgpu::present::{GpuSetup, OpenDevice, OpenGpu, open_vulkan_device};
pub use beui_renderer_wgpu::{Renderer, Repaint, Repainting, renderer_info, wgpu_color};

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use crate::runner::{native_platform, run_on, run_with, run_with_device};
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use crate::web::{run_web, web_platform};

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
