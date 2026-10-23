use std::time::Duration;

use crate::reactive::{
    Callback, Child, Children, Get, Placed, Prop, create_effect, create_memo, create_timer,
    with_document,
};
use beui_core::base::canvas::{CanvasNode, CanvasView};
use beui_core::document::Document;
use beui_core::geometry::{Rect, Vec2, pos2};
use beui_core::node::NodeId;
use beui_macros::component;

pub type CanvasItem = Placed<CanvasNode>;

const RASTER_SETTLE: Duration = Duration::from_millis(150);

#[component]
pub fn Canvas(
    #[prop(default = None)] view: Prop<Option<CanvasView>>,
    #[prop(default = 0.0)] width: Prop<f32>,
    #[prop(default = 0.0)] height: Prop<f32>,
    #[prop(default = false)] scrolls_across: bool,
    on_reveal: Callback<Vec2>,
    children: Children<CanvasItem>,
) -> NodeId {
    let canvas = with_document(Document::create_canvas);
    with_document(|document| {
        document.set_canvas_scrolls_across(canvas, scrolls_across);
        document.set_canvas_on_reveal(canvas, on_reveal);
    });
    children.mount(canvas);
    create_effect(move || {
        let view = view.get();
        with_document(|document| document.set_canvas_view(canvas, view))
    });
    let scale = create_memo(move || view.get().map_or(1.0, |view| view.scale));
    let settle = create_timer(move || {
        let raster = scale.get_untracked();
        with_document(|document| document.set_canvas_raster(canvas, Some(raster)));
        None
    });
    create_effect(move || {
        let scale = scale.get();
        let unset = with_document(|document| document.canvas_raster(canvas).is_none());
        match unset {
            true => with_document(|document| document.set_canvas_raster(canvas, Some(scale))),
            false => settle.restart(RASTER_SETTLE),
        }
    });
    create_effect(move || {
        let size = Vec2::new(width.get(), height.get());
        with_document(|document| document.set_canvas_size(canvas, size));
    });
    canvas.id()
}

#[component]
pub fn CanvasItem(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    #[prop(default = true)] clip: Prop<bool>,
    #[prop(default = true)] pointer: Prop<bool>,
    #[prop(default = false)] scales: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    let item = with_document(|document| {
        let item = document.create_canvas_item();
        if let Some(child) = children {
            document.set_canvas_item_child(item, child);
        }
        item
    });
    create_effect(move || {
        let rect =
            Rect::from_min_size(pos2(x.get(), y.get()), Vec2::new(width.get(), height.get()));
        with_document(|document| document.set_canvas_item_rect(item, rect));
    });
    create_effect(move || {
        let clip = clip.get();
        with_document(|document| document.set_canvas_item_clip(item, clip))
    });
    create_effect(move || {
        let pointer = pointer.get();
        with_document(|document| document.set_canvas_item_pointer(item, pointer))
    });
    create_effect(move || {
        let scales = scales.get();
        with_document(|document| document.set_canvas_item_scales(item, scales))
    });
    CanvasItem::new(item.id(), ())
}
