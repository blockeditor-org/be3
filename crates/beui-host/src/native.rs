/// The size of the on-screen keyboard's window, which `build` installs.
pub use crate::keyboard_host::WINDOW_SIZE as KEYBOARD_WINDOW_SIZE;
pub use beui::native::*;
#[cfg(not(target_arch = "wasm32"))]
pub use beui_font_skrifa::system::SystemFonts;
pub use beui_font_skrifa::{
    FontBytes, FontData, FontLibrary, FontSources, ICONS_FONT, SkrifaFonts,
};
pub use beui_inspector::install as install_inspector;
pub use beui_layer_drive as drive;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub use beui_platform_headless::Headless;
pub use beui_platform_headless::Simulation;
pub use beui_platform_headless::testing;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use beui_platform_web::accessibility_tree;
#[cfg(not(target_arch = "wasm32"))]
pub use beui_renderer_wgpu::present::read_texture;
pub use beui_renderer_wgpu::present::{
    GpuSetup, OpenDevice, OpenGpu, browser_features, open_vulkan_device,
};
pub use beui_renderer_wgpu::{Renderer, Repaint, Repainting, Resources, renderer_info, wgpu_color};
pub use beui_runner::environment;
pub use beui_runner::{
    Incoming, Launch, Layer, LayerFrame, LayerView, Platform, PlatformServices, RunOptions,
    Running, Shown, Units,
};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use crate::web::{run_web, web_platform};

pub fn build(f: impl FnOnce() -> beui_view::primitives::Root) -> Document {
    let mut document = beui_view::native_api::build(f);
    beui_inspector::install(&mut document);
    crate::keyboard_host::install_keyboard(&mut document);
    document
}

pub fn context() -> Context {
    Context::new(SkrifaFonts::default())
}

pub fn system_fonts() -> FontLibrary {
    let fonts = FontLibrary::bundled();
    #[cfg(not(target_arch = "wasm32"))]
    let fonts = fonts.with_fallback(beui_font_skrifa::system::fallback());
    fonts
}

pub fn system_context() -> Context {
    Context::new(SkrifaFonts::new(system_fonts()))
}
