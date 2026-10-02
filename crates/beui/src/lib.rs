extern crate self as beui;

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
use std::error::Error;

pub use accesskit;
pub use beui_components_styled as styled;
pub use beui_components_unstyled as unstyled;
pub use beui_components_unstyled::datetime;
pub use beui_core::app::{App, Setup, Waker};
pub use beui_core::base::{
    Align, Direction, ImeCursor, ItemSize, Justify, ScrollPosition, Sizing, Track, focus_within,
};
pub use beui_core::color::{Color32, Hsva, format_hex, parse_hex};
pub use beui_core::context::{
    Context, FrameOutput, InputSimulation, Moved, RendererChoices, RendererInfo,
};
pub use beui_core::damage::Region;
pub use beui_core::document::{
    Document, OverRepaint, Tools, detect_over_repaint, take_over_repaints, verify_paint,
};
pub use beui_core::draw::{Quad, Quads, Turn, quads};
pub use beui_core::drawing::Drawing;
pub use beui_core::file_picker::{FileFilter, FilePick, FilePickId, FilePickRequest, PickedFile};
pub use beui_core::filter::{ColorVision, Filter, MAX_BLUR};
pub use beui_core::font::{
    FontBackend, FontFamily, FontId, Galley, GalleyLine, Glyph, GlyphId, GlyphImage, Shaping,
    TextAlign, TextLayout, Wraps, line_height,
};
pub use beui_core::geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
pub use beui_core::icons;
pub use beui_core::image::{Image, ImageFit, ImageId, Thumbhash};
pub use beui_core::input::{
    AutoscrollGesture, BackEdge, BackGesture, CursorIcon, DroppedFile, Event, ImeArea, ImeEvent,
    ImeText, InputState, Key, KeyPress, Modifiers, PointerButton, PointerPress, RawInput,
    ScrollGesture, SecondaryDrag, TouchId, TouchPhase, TouchPoint, TouchState, ZoomGesture,
};
pub use beui_core::interact::forward::ForwardedInput;
pub use beui_core::node::{ClickHandler, Handler, NodeId, NodeOf};
pub use beui_core::page::{Page, PageShape};
pub use beui_core::painter::{Corners, Painter, Shape};
pub use beui_core::performance::{FramePerformance, PerformanceSnapshot, PerformanceTimings};
#[cfg(not(target_arch = "wasm32"))]
pub use beui_font_freetype::system::SystemFonts;
pub use beui_font_freetype::{
    FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts, ICONS_FONT,
};
pub use beui_inspector::install as install_inspector;
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::present::{GpuSetup, OpenDevice};
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::{
    Draw, DrawAt, Renderer, Repaint, Repainting, clear_color, drawing, renderer_info,
};
pub use beui_view::{child_type, value_child_type};

#[cfg(all(feature = "window", target_os = "android"))]
pub use beui_adapter_android::{AndroidApp, RunOptions};
#[cfg(all(any(feature = "web", feature = "dom"), target_arch = "wasm32"))]
pub use beui_adapter_web::{RunOptions, accessibility_tree};
#[cfg(all(feature = "window", not(target_os = "android")))]
pub use beui_adapter_winit::{RunOptions, winit};
#[cfg(all(any(feature = "web", feature = "dom"), target_arch = "wasm32"))]
pub use web::{WebRenderer, run_web};

#[cfg(all(any(feature = "web", feature = "dom"), target_arch = "wasm32"))]
mod web;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use window::{WindowRenderer, run_with, run_with_renderers};

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod window;

pub mod reactive {
    pub use beui_components_unstyled::Button;
    pub use beui_view::reactive::*;

    use beui_core::document::Document;
    use beui_core::node::NodeId;

    pub fn build(f: impl FnOnce() -> NodeId) -> Document {
        let mut document = beui_view::reactive::build(f);
        beui_inspector::install(&mut document);
        document
    }
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
pub fn run(title: impl Into<String>, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::new(title), app)
}

#[cfg(test)]
use beui_core::{
    accessibility, base, color, context, culling, damage, draw, drawing, filter, flash, font,
    geometry, image, input, interact, node, painter, screen_simulation,
};
#[cfg(test)]
use beui_inspector::{self as inspector, mouse_simulation};
#[cfg(test)]
use beui_renderer_wgpu as renderer;

#[cfg(test)]
mod tests;
