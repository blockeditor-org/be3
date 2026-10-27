use crate::reactive::Child;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::types::Align2;
use crate::types::Color;
use crate::types::Corners;
use crate::types::Extent;
use crate::types::Outline;
use crate::types::Sides;
use crate::types::Vec2;
use beui_core_html::css;
use beui_core_html::document::{append, create, set_style};

use super::bind;

fn bind_extent(node: NodeId, extent: Prop<Extent>, properties: [&'static str; 3]) {
    bind(extent, move |extent| {
        let length = css::extent(extent);
        set_style(node, properties[0], length.size);
        set_style(node, properties[1], length.min);
        set_style(node, properties[2], length.max);
    });
}

pub fn frame(
    width: Prop<Extent>,
    height: Prop<Extent>,
    aspect_ratio: Prop<Option<f32>>,
    padding: Prop<Sides>,
    align: Prop<Align2>,
    fill: Prop<Color>,
    radius: Prop<Corners>,
    outline: Prop<Option<Outline>>,
    translate: Prop<Vec2>,
    rotate: Prop<f32>,
    opacity: Prop<f32>,
    fade_edges: Prop<Sides>,
    children: Option<Child>,
) -> NodeId {
    let frame = create("Frame", "div");
    set_style(frame, "display", Some("grid".to_owned()));
    set_style(
        frame,
        "grid-template",
        Some("minmax(0, 1fr) / minmax(0, 1fr)".to_owned()),
    );
    if let Some(child) = children {
        append(frame, child);
    }
    bind_extent(frame, width, ["width", "min-width", "max-width"]);
    bind_extent(frame, height, ["height", "min-height", "max-height"]);
    bind(aspect_ratio, move |ratio| {
        set_style(frame, "aspect-ratio", ratio.map(css::number));
    });
    bind(padding, move |padding| {
        set_style(
            frame,
            "padding",
            (padding != Sides::ZERO).then(|| css::sides(padding)),
        );
    });
    bind(align, move |align| {
        set_style(
            frame,
            "justify-items",
            Some(css::align(align.horizontal).to_owned()),
        );
        set_style(
            frame,
            "align-items",
            Some(css::align(align.vertical).to_owned()),
        );
    });
    bind(fill, move |fill| {
        set_style(
            frame,
            "background-color",
            (!fill.is_clear()).then(|| css::color(fill)),
        );
    });
    bind(radius, move |radius| {
        set_style(
            frame,
            "border-radius",
            (radius != Corners::ZERO).then(|| css::corners(radius)),
        );
    });
    bind(outline, move |outline| {
        let (line, offset) = outline.map(css::outline).unzip();
        set_style(frame, "outline", line);
        set_style(frame, "outline-offset", offset);
    });
    create_binding(move || {
        let (translate, rotate) = (translate.get(), rotate.get());
        let mut transform = Vec::new();
        if translate != Vec2::ZERO {
            transform.push(format!(
                "translate({}, {})",
                css::px(translate.x),
                css::px(translate.y)
            ));
        }
        if rotate != 0.0 {
            transform.push(format!("rotate({}rad)", css::number(rotate)));
        }
        set_style(
            frame,
            "transform",
            (!transform.is_empty()).then(|| transform.join(" ")),
        );
    });
    bind(opacity, move |opacity| {
        set_style(
            frame,
            "opacity",
            (opacity < 1.0).then(|| css::number(opacity.max(0.0))),
        );
    });
    bind(fade_edges, move |fade| {
        let mask = css::fade(fade);
        let layers = mask
            .as_ref()
            .map_or(0, |mask| mask.matches("linear-gradient").count());
        set_style(frame, "mask-image", mask.clone());
        set_style(frame, "-webkit-mask-image", mask);
        set_style(
            frame,
            "mask-composite",
            (layers > 1).then(|| "intersect".to_owned()),
        );
    });
    frame
}
