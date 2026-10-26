extern crate self as beui;

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
use std::error::Error;

pub use accesskit;
pub use beui_components_styled as styled;
pub use beui_components_unstyled as unstyled;
pub use beui_components_unstyled::datetime;
pub use beui_core::accessibility;
pub use beui_core::animation;
pub use beui_core::app::accessibility_dump;
pub use beui_core::app::{App, FrameTimes, Lifecycle, Setup, Waker};
pub use beui_core::base::overlay::{OverlayAnchor, Placement};
pub use beui_core::base::turn::TurnNode;
pub use beui_core::base::{
    Align, Direction, ItemSize, Justify, ScrollPosition, Sides, Sizing, Track,
};
pub use beui_core::clipboard::{PasteAnswer, PasteKind, PasteRequest, Pasted};
pub use beui_core::color::{Color32, Hsva, Oklch, format_hex, parse_hex};
pub use beui_core::context::{
    ActionGroup, ActionInfo, Context, FrameOutput, InputSimulation, Moved, RendererInfo,
    WindowOutput,
};
pub use beui_core::damage::Region;
pub use beui_core::document::{
    OverRepaint, Tools, detect_over_repaint, fail_unsettled_layout, take_over_repaints,
    verify_paint,
};
pub use beui_core::draw::{Quad, Quads, Turn, quads};
pub use beui_core::drawing::Drawing;
pub use beui_core::drawn_cursor;
pub use beui_core::file_picker::{
    FileFilter, FilePick, FilePickAnswer, FilePickId, FilePickRequest, PickedFile,
};
pub use beui_core::file_saver::{FileSave, FileSaveAnswer, FileSaveId, FileSaveRequest, SavedFile};
pub use beui_core::filter::{ColorVision, Filter, MAX_BLUR};
pub use beui_core::focus::{ImeCursor, focus_within};
pub use beui_core::font::{
    FontBackend, FontFamily, FontId, Galley, GalleyLine, Glyph, GlyphId, GlyphImage, Overflow,
    Shaping, TextAlign, TextLayout, WrapAt, Wraps, line_height,
};
pub use beui_core::geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
pub use beui_core::icons;
pub use beui_core::image::{Image, ImageFit, ImageId, Thumbhash};
pub use beui_core::input::{
    AutoscrollGesture, BackEdge, BackGesture, CursorIcon, DroppedFile, Event, ImeArea, ImeEvent,
    ImeText, Input, InputState, Key, KeyChord, KeyPress, Keyboard, Keystroke, Modifiers, Pen,
    PenPhase, PointerButton, PointerEvent, PointerId, PointerPhase, PointerPress, ScrollDelta,
    ScrollGesture, SecondaryDrag, TouchId, TouchPhase, ZoomGesture,
};
pub use beui_core::interact::forward::{Aim, Forwarded, ForwardedInput, Over, Replay, Replayed};
pub use beui_core::keys::{KeyAt, KeyEvent, KeyInput, KeyPhase, KeyTarget};
pub use beui_core::layer::{Dismiss, LayerLevel, LayerPolicy};
pub use beui_core::node::{ClickHandler, Handler, NodeId, NodeOf};
pub use beui_core::overflow;
pub use beui_core::page::{Page, PageShape};
pub use beui_core::painter::{Corners, Painter, Shape};
pub use beui_core::painting;
pub use beui_core::path::{
    FillRule, LineCap, LineJoin, Path, PathElement, PathPaint, PathStyle, Stroke,
};
pub use beui_core::performance::{
    FramePerformance, FrameWork, PerformanceSnapshot, PerformanceTimings,
};
pub use beui_core::preferences::{
    CanvasTouch, PreferenceLayer, PreferenceLayers, PreferenceSource, Preferences, ThemeKind,
};
pub use beui_core::rich;
pub use beui_core::runner::environment;
pub use beui_core::touch_cursor;
pub use beui_dock as dock;
#[cfg(not(target_arch = "wasm32"))]
pub use beui_font_freetype::system::SystemFonts;
pub use beui_font_freetype::{
    FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts, ICONS_FONT,
};
pub use beui_inspector::install as install_inspector;
pub use beui_keyboard as keyboard;
pub use beui_layer_drive as drive;
pub use beui_platform_headless::testing;
#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
pub use beui_renderer_wgpu::present::read_texture;
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::present::{GpuSetup, OpenDevice, OpenGpu, open_vulkan_device};
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::{
    Draw, DrawAt, Renderer, Repaint, Repainting, drawing, painted_drawing, renderer_info,
    wgpu_color,
};
pub use beui_view::primitives;
pub use beui_view::window::{Window, WindowHandle, opens_windows, use_window};
pub use beui_view::types::{
    Align2, CaretHandle, Extent, Outline, SpanBackground, SpanStyle, TextCaret, TextGlyph, TextMark,
};
pub use beui_view::{CORE, Core, child_type, value_child_type};

pub use beui_core::runner::{
    Incoming, Launch, Layer, LayerFrame, LayerView, Platform, PlatformServices, RunOptions,
    Running, Shown, Units, panic_message,
};
pub use beui_core::window::{
    HostPlacement, Resize, WindowEdge, WindowHosting, WindowId, WindowPlacement, WindowReport,
    WindowRequest, WindowRole, WindowSize,
};
#[cfg(all(feature = "window", target_os = "android"))]
pub use beui_platform_android::AndroidApp;
pub use beui_platform_headless::Simulation;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use beui_platform_web::accessibility_tree;
#[cfg(all(feature = "window", not(target_os = "android")))]
pub use beui_platform_winit::winit;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::{run_web, web_platform};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;

mod keyboard_host;
pub mod native;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use window::{native_platform, run_on, run_with, run_with_device};

#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod saved_preferences;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod window;

#[cfg(all(
    feature = "window",
    not(any(target_os = "android", target_arch = "wasm32"))
))]
pub use beui_platform_headless::Headless;

pub mod reactive {
    pub use beui_view::reactive::*;

    use beui_core::document::Document;

    pub fn build(f: impl FnOnce() -> beui_view::primitives::Root) -> Document {
        let mut document = beui_view::reactive::build(f);
        beui_inspector::install(&mut document);
        crate::keyboard_host::install_keyboard(&mut document);
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
pub fn run(app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::default(), app)
}

#[cfg(test)]
use beui_core::{
    base, color, context, damage, draw, drawing, filter, flash, font, geometry, image, input,
    interact, node, painter, screen_simulation, sight,
};
#[cfg(test)]
use beui_inspector::{self as inspector, mouse_simulation};
#[cfg(test)]
use beui_renderer_wgpu as renderer;

#[cfg(test)]
mod tests;
