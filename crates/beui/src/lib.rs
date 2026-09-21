extern crate self as beui;

mod accessibility;
#[cfg(feature = "window")]
mod app;
mod base;
mod color;
mod context;
mod damage;
mod document;
mod draw;
mod drawing;
mod filter;
mod flash;
mod font;
mod geometry;
pub mod icons;
mod image;
mod input;
mod inspector;
mod interact;
mod layout;
mod mouse_simulation;
mod node;
mod paint;
mod painter;
mod performance;
mod pixel_grid;
pub mod reactive;
#[cfg(feature = "render")]
mod renderer;
mod screen_reader;
pub mod styled;
pub mod unstyled;

pub use accesskit;
#[cfg(feature = "window")]
pub use app::{App, run};
pub use base::{Align, Direction, ItemSize, ScrollPosition, TextAlign, focus_within};
pub use color::Color32;
pub use context::{Context, FrameOutput};
pub use document::Document;
pub use draw::{Quad, Quads, Turn, quads, quads_within};
pub use drawing::Drawing;
#[cfg(feature = "render")]
pub use drawing::{Draw, DrawAt};
pub use filter::{ColorVision, Filter, MAX_BLUR};
pub use font::{
    FontFamily, FontId, FontSources, Galley, Glyph, GlyphId, GlyphImage, ICONS_FONT, TextLayout,
    line_height,
};
pub use geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
pub use image::{Image, ImageFit, ImageId};
pub use input::{
    CursorIcon, Event, InputState, Key, KeyPress, Modifiers, PointerButton, PointerPress, RawInput,
    ScrollGesture, TouchId, TouchPhase, TouchPoint, TouchState, ZoomGesture,
};
pub use node::{ClickHandler, Handler, NodeId};
pub use painter::{Painter, Shape};
pub use performance::{FramePerformance, PerformanceSnapshot, PerformanceTimings};
#[cfg(feature = "render")]
pub use renderer::{Renderer, Repaint, clear_color};
