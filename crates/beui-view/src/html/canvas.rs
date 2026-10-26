use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Placed;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::types::CanvasView;
use crate::types::Vec2;
use beui_core_html::css;
use beui_core_html::document::{append, create, set_style};
use beui_core_html::tree::{CanvasKind, host};

use super::bind;

pub type CanvasItem = Placed<CanvasKind>;

pub fn canvas(
    view: Prop<Option<CanvasView>>,
    width: Prop<f32>,
    height: Prop<f32>,
    scrolls_across: bool,
    on_reveal: Callback<Vec2>,
    children: Children<CanvasItem>,
) -> NodeId {
    let _ = (view, scrolls_across, on_reveal);
    let canvas = super::stand_in("Canvas");
    set_style(canvas, "position", Some("relative".to_owned()));
    set_style(canvas, "overflow", Some("hidden".to_owned()));
    create_binding(move || {
        let (width, height) = (width.get(), height.get());
        set_style(canvas, "min-width", (width > 0.0).then(|| css::px(width)));
        set_style(
            canvas,
            "min-height",
            (height > 0.0).then(|| css::px(height)),
        );
    });
    children.mount(host::<CanvasKind>(canvas));
    canvas
}

pub fn canvas_item(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    clip: Prop<bool>,
    pointer: Prop<bool>,
    scales: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    let _ = scales;
    let item = create("CanvasItem", "div");
    set_style(item, "position", Some("absolute".to_owned()));
    if let Some(child) = children {
        append(item, child);
    }
    bind(x, move |x| set_style(item, "left", Some(css::px(x))));
    bind(y, move |y| set_style(item, "top", Some(css::px(y))));
    bind(width, move |width| {
        set_style(item, "width", Some(css::px(width)))
    });
    bind(height, move |height| {
        set_style(item, "height", Some(css::px(height)))
    });
    bind(clip, move |clip| {
        set_style(item, "overflow", clip.then(|| "hidden".to_owned()));
    });
    bind(pointer, move |pointer| {
        set_style(
            item,
            "pointer-events",
            (!pointer).then(|| "none".to_owned()),
        );
    });
    CanvasItem::new(item, ())
}
