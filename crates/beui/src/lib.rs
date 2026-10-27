extern crate self as beui;

#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
use std::error::Error;

pub use accesskit;
pub use beui_components_styled as styled;
pub use beui_components_unstyled as unstyled;
pub use beui_components_unstyled::datetime;
#[cfg(not(beui_html))]
pub use beui_core::accessibility;
#[cfg(not(beui_html))]
pub use beui_core::app::accessibility_dump;
#[cfg(not(beui_html))]
pub use beui_core::app::{App, FrameTimes, Lifecycle, NativeWindow, Setup, Waker};
#[cfg(not(beui_html))]
pub use beui_core::base::overlay::{OverlayAnchor, Placement};
#[cfg(not(beui_html))]
pub use beui_core::base::{
    Align, Direction, ItemSize, Justify, ScrollPosition, Sides, Sizing, Track,
};
#[cfg(not(beui_html))]
pub use beui_core::clipboard::{PasteAnswer, PasteKind, PasteRequest, Pasted};
#[cfg(not(beui_html))]
pub use beui_core::color::{
    Color, Color32, ColorDefinitions, ColorScheme, ColorSource, ColorVar, Hsva, Oklch,
    SchemeColors, format_hex, parse_hex,
};
#[cfg(not(beui_html))]
pub use beui_core::context::{
    ActionGroup, ActionInfo, Context, FrameOutput, InputSimulation, Moved, RendererInfo,
    WindowOutput,
};
#[cfg(not(beui_html))]
pub use beui_core::damage::Region;
#[cfg(not(beui_html))]
pub use beui_core::document::{
    OverRepaint, Tools, detect_over_repaint, fail_unsettled_layout, take_over_repaints,
    verify_paint,
};
#[cfg(not(beui_html))]
pub use beui_core::draw::{Quad, Quads, Turn, quads};
#[cfg(not(beui_html))]
pub use beui_core::drawing::Drawing;
#[cfg(not(beui_html))]
pub use beui_core::drawn_cursor;
#[cfg(not(beui_html))]
pub use beui_core::file_picker::{
    FileFilter, FilePick, FilePickAnswer, FilePickId, FilePickRequest, PickedFile,
};
#[cfg(not(beui_html))]
pub use beui_core::file_saver::{FileSave, FileSaveAnswer, FileSaveId, FileSaveRequest, SavedFile};
#[cfg(not(beui_html))]
pub use beui_core::filter::{ColorVision, Filter, MAX_BLUR};
#[cfg(not(beui_html))]
pub use beui_core::focus::{ImeCursor, focus_within};
#[cfg(not(beui_html))]
pub use beui_core::font::{
    FontBackend, FontFamily, FontId, Galley, GalleyLine, Glyph, GlyphId, GlyphImage, Overflow,
    Shaping, TextAlign, TextLayout, WrapAt, Wraps, line_height,
};
#[cfg(not(beui_html))]
pub use beui_core::geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
#[cfg(not(beui_html))]
pub use beui_core::icons;
#[cfg(not(beui_html))]
pub use beui_core::image::{Image, ImageFit, ImageId, Thumbhash};
#[cfg(not(beui_html))]
pub use beui_core::input::{
    AutoscrollGesture, BackEdge, BackGesture, CursorIcon, DroppedFile, Event, ImeArea, ImeEvent,
    ImeText, Input, InputState, Key, KeyChord, KeyPress, Keyboard, Keystroke, Modifiers, Pen,
    PenPhase, PointerButton, PointerEvent, PointerId, PointerPhase, PointerPress, ScrollDelta,
    ScrollGesture, SecondaryDrag, SoftKeyboard, TouchId, TouchPhase, ZoomGesture,
};
#[cfg(not(beui_html))]
pub use beui_core::interact::forward::{Aim, Forwarded, ForwardedInput, Over, Replay, Replayed};
#[cfg(not(beui_html))]
pub use beui_core::keys::{KeyAt, KeyEvent, KeyInput, KeyPhase, KeyTarget};
#[cfg(not(beui_html))]
pub use beui_core::layer::{Dismiss, LayerLevel, LayerPolicy};
#[cfg(not(beui_html))]
pub use beui_core::node::{ClickHandler, Handler, NodeId, NodeOf};
#[cfg(not(beui_html))]
pub use beui_core::overflow;
#[cfg(not(beui_html))]
pub use beui_core::page::{Page, PageShape};
#[cfg(not(beui_html))]
pub use beui_core::painter::{Corners, Painter, Shape};
#[cfg(not(beui_html))]
pub use beui_core::painting;
#[cfg(not(beui_html))]
pub use beui_core::path::{
    FillRule, LineCap, LineJoin, Path, PathElement, PathPaint, PathStyle, Stroke,
};
#[cfg(not(beui_html))]
pub use beui_core::performance::{
    FramePerformance, FrameWork, PerformanceSnapshot, PerformanceTimings,
};
#[cfg(not(beui_html))]
pub use beui_core::preferences::{
    CanvasTouch, PreferenceLayer, PreferenceLayers, PreferenceSource, Preferences, ThemeKind,
};
#[cfg(not(beui_html))]
pub use beui_core::rich;
#[cfg(not(beui_html))]
pub use beui_core::runner::environment;
#[cfg(not(beui_html))]
pub use beui_core::touch_cursor;
#[cfg(not(beui_html))]
pub use beui_dock as dock;
#[cfg(not(beui_html))]
#[cfg(not(target_arch = "wasm32"))]
pub use beui_font_freetype::system::SystemFonts;
#[cfg(not(beui_html))]
pub use beui_font_freetype::{
    FontBytes, FontData, FontLibrary, FontSources, FreetypeFonts, ICONS_FONT,
};
#[cfg(not(beui_html))]
pub use beui_inspector::install as install_inspector;
#[cfg(not(beui_html))]
pub use beui_keyboard as keyboard;
#[cfg(not(beui_html))]
pub use beui_layer_drive as drive;
#[cfg(not(beui_html))]
pub use beui_platform_headless::testing;
#[cfg(not(beui_html))]
#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
pub use beui_renderer_wgpu::present::read_texture;
#[cfg(not(beui_html))]
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::present::{GpuSetup, OpenDevice, OpenGpu, open_vulkan_device};
#[cfg(not(beui_html))]
#[cfg(feature = "render")]
pub use beui_renderer_wgpu::{
    Draw, DrawAt, Renderer, Repaint, Repainting, drawing, painted_drawing, renderer_info,
    wgpu_color,
};
pub use beui_view::animation;
pub use beui_view::components::selectable;
pub use beui_view::primitives;
pub use beui_view::types::{
    Align2, Area, CanvasView, CaretHandle, Claim, Extent, HitRegion, Outline, PointerInput,
    PointerStage, SpanBackground, SpanStyle, TextCaret, TextGlyph, TextMark, WheelInput, color_var,
};
#[cfg(not(beui_html))]
pub use beui_view::window::{Window, WindowHandle, opens_windows, use_window};
pub use beui_view::{CORE, Core, child_type, value_child_type};

#[cfg(not(beui_html))]
pub use beui_core::runner::{
    Incoming, Launch, Layer, LayerFrame, LayerView, Platform, PlatformServices, RunOptions,
    Running, Shown, Units, panic_message,
};
#[cfg(not(beui_html))]
pub use beui_core::window::{
    HostPlacement, Resize, WindowEdge, WindowHosting, WindowId, WindowPlacement, WindowReport,
    WindowRequest, WindowRole, WindowSize,
};
#[cfg(not(beui_html))]
#[cfg(all(feature = "window", target_os = "android"))]
pub use beui_platform_android::AndroidApp;
#[cfg(not(beui_html))]
pub use beui_platform_headless::Simulation;
#[cfg(not(beui_html))]
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use beui_platform_web::accessibility_tree;
#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_os = "android")))]
pub use beui_platform_winit::winit;
#[cfg(not(beui_html))]
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::{run_web, web_platform};

#[cfg(not(beui_html))]
#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;

#[cfg(not(beui_html))]
mod keyboard_host;
#[cfg(not(beui_html))]
pub mod native;
#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use window::{native_platform, run_on, run_with, run_with_device};

#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod saved_preferences;
#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod window;

#[cfg(not(beui_html))]
#[cfg(all(
    feature = "window",
    not(any(target_os = "android", target_arch = "wasm32"))
))]
pub use beui_platform_headless::Headless;

#[cfg(not(beui_html))]
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

#[cfg(not(beui_html))]
pub fn context() -> Context {
    Context::new(FreetypeFonts::default())
}

#[cfg(not(beui_html))]
pub fn system_fonts() -> FontLibrary {
    let fonts = FontLibrary::bundled();
    #[cfg(not(target_arch = "wasm32"))]
    let fonts = fonts.with_fallback(beui_font_freetype::system::fallback());
    fonts
}

#[cfg(not(beui_html))]
pub fn system_context() -> Context {
    Context::new(FreetypeFonts::new(system_fonts()))
}

#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub fn run(app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with(RunOptions::default(), app)
}

#[cfg(not(beui_html))]
#[cfg(test)]
use beui_core::{
    base, color, context, damage, draw, drawing, filter, flash, font, geometry, image, input,
    interact, node, painter, screen_simulation, sight,
};
#[cfg(not(beui_html))]
#[cfg(test)]
use beui_inspector::{self as inspector, mouse_simulation};
#[cfg(not(beui_html))]
#[cfg(test)]
use beui_renderer_wgpu as renderer;

#[cfg(not(beui_html))]
#[cfg(test)]
mod tests;

#[cfg(beui_html)]
pub use beui_view::reactive::{KeyAt, KeyEvent, NodeId, OverlayAnchor};
#[cfg(beui_html)]
pub use beui_view::types::*;

#[cfg(beui_html)]
mod html_paintings;

#[cfg(beui_html)]
pub mod reactive {
    pub use beui_view::reactive::*;
}

#[cfg(beui_html)]
pub mod html {
    pub use beui_view::running::{
        STYLES_ID, choose_theme, render_to_string, stylesheet,
    };
    #[cfg(target_arch = "wasm32")]
    pub use beui_view::running::{automate, hydrate, run};
}
