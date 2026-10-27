use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::types::CanvasView;
use crate::types::Vec2;
use crate::types::tree::CanvasItem;
use beui_core_html::canvas::scale_property;
use beui_core_html::css;
use beui_core_html::document::{append, create, set_style};
use beui_core_html::tree::{CanvasKind, host};

use super::bind;

pub(crate) fn canvas(
    view: Prop<Option<CanvasView>>,
    width: Prop<f32>,
    height: Prop<f32>,
    scrolls_across: bool,
    on_reveal: Callback<Vec2>,
    children: Children<CanvasItem>,
) -> NodeId {
    let _ = scrolls_across;
    let canvas = create("Canvas", "div");
    for (property, value) in [("position", "relative"), ("overflow", "hidden")] {
        set_style(canvas, property, Some(value.to_owned()));
    }
    let shown = create("CanvasView", "div");
    for (property, value) in [
        ("position", "absolute"),
        ("left", "0"),
        ("top", "0"),
        ("width", "0"),
        ("height", "0"),
        ("transform-origin", "0 0"),
    ] {
        set_style(shown, property, Some(value.to_owned()));
    }
    append(canvas, shown);
    create_binding(move || {
        let (width, height) = (width.get(), height.get());
        set_style(canvas, "min-width", (width > 0.0).then(|| css::px(width)));
        set_style(
            canvas,
            "min-height",
            (height > 0.0).then(|| css::px(height)),
        );
    });
    let state = beui_core_html::canvas::canvas(canvas, shown, on_reveal);
    bind(view, move |view| state.set_view(view));
    children.mount(host::<CanvasKind>(shown));
    canvas
}

pub(crate) fn canvas_item(
    x: Prop<f32>,
    y: Prop<f32>,
    width: Prop<f32>,
    height: Prop<f32>,
    clip: Prop<bool>,
    pointer: Prop<bool>,
    scales: Prop<bool>,
    children: Option<Child>,
) -> CanvasItem {
    let item = create("CanvasItem", "div");
    set_style(item, "position", Some("absolute".to_owned()));
    set_style(item, "display", Some("grid".to_owned()));
    set_style(
        item,
        "grid-template",
        Some("minmax(0, 1fr) / minmax(0, 1fr)".to_owned()),
    );
    beui_core_html::canvas::item(item);
    if let Some(child) = children {
        append(item, child);
    }
    bind(x, move |x| set_style(item, "left", Some(scale_property(x))));
    bind(y, move |y| set_style(item, "top", Some(scale_property(y))));
    create_binding(move || {
        let (width, height, scales) = (width.get(), height.get(), scales.get());
        let length = |length: f32| match scales {
            true => css::px(length),
            false => scale_property(length),
        };
        set_style(item, "width", Some(length(width)));
        set_style(item, "height", Some(length(height)));
        set_style(
            item,
            "transform",
            scales.then(|| "scale(var(--beui-scale, 1))".to_owned()),
        );
        set_style(item, "transform-origin", scales.then(|| "0 0".to_owned()));
    });
    bind(clip, move |clip| {
        beui_core_html::canvas::set_item_clip(item, clip)
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
