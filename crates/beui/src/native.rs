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
pub use beui_core::touch_cursor;
pub use beui_core::window::{Resize, WindowHosting, WindowSize};
pub use beui_draw::{Draw, DrawAt, drawing, painted_drawing};
pub use beui_view::native_api::{core_painter, draw_gpu, wrap_painter};
pub use embed::Embed;

mod embed;
