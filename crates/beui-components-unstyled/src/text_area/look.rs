use beui_macros::view;

use beui_view::components::text::Span;
use beui_view::primitives::*;
use beui_view::reactive::*;

use super::hooks::{LeafSpan, PlainLine, TextLeaf, TextLine, TextMarker};

#[derive(Clone, Copy)]
pub struct TextAreaLook {
    pub leaf: RenderFn<TextLeaf, Span>,
    pub marker: RenderFn<TextMarker>,
    pub line: RenderFn<TextLine>,
    pub geometry: Prop<TextAreaGeometry>,
}

impl TextAreaLook {
    pub fn plain(geometry: Prop<TextAreaGeometry>) -> Self {
        Self {
            leaf: RenderFn::new(|leaf: TextLeaf| {
                view! {
                    <LeafSpan leaf />
                }
            }),
            marker: RenderFn::new(|marker: TextMarker| {
                let gap = marker.gap();
                view! {
                    <Frame padding=gap />
                }
            }),
            line: RenderFn::new(|line: TextLine| {
                view! {
                    <PlainLine line />
                }
            }),
            geometry,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextAreaGeometry {
    pub caret_width: f32,
    pub line_padding: (f32, f32),
    pub end_room: f32,
    pub gutter_padding: (f32, f32),
    pub gutter_arrow: f32,
    pub ellipsis_gap: f32,
    pub run_chip: ChipGeometry,
    pub widget: WidgetGeometry,
    pub blocks: BlockGeometry,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChipGeometry {
    pub height: f32,
    pub padding: f32,
    pub radius: f32,
    pub font_size: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WidgetGeometry {
    pub height: f32,
    pub radius: f32,
    pub icon_width: f32,
    pub icon_size: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BlockGeometry {
    pub heading_space: [f32; 3],
    pub quote_space: f32,
    pub code_space: f32,
    pub table_space: f32,
    pub rule_gap: f32,
    pub cell_padding: f32,
    pub delimiter_size: f32,
}
