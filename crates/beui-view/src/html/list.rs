use crate::reactive::Children;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Align;
use crate::types::Direction;
use crate::types::Justify;
use beui_core_html::css;
use beui_core_html::document::{create, set_direction, set_style};
use beui_core_html::tree::{ListKind, host};

use super::bind;

pub(crate) fn list(
    direction: Prop<Direction>,
    align: Prop<Align>,
    justify: Prop<Justify>,
    wrap: Prop<bool>,
    spacing: Prop<f32>,
    children: Children<crate::reactive::ListChild>,
) -> NodeId {
    let list = create("List", "div");
    set_style(list, "display", Some("flex".to_owned()));
    bind(direction, move |direction| {
        set_direction(list, direction);
        set_style(
            list,
            "flex-direction",
            Some(css::flex_direction(direction).to_owned()),
        );
    });
    bind(align, move |align| {
        set_style(list, "align-items", Some(css::align(align).to_owned()));
    });
    bind(justify, move |justify| {
        set_style(
            list,
            "justify-content",
            (justify != Justify::Start).then(|| css::justify(justify).to_owned()),
        );
    });
    bind(wrap, move |wrap| {
        set_style(list, "flex-wrap", wrap.then(|| "wrap".to_owned()));
    });
    bind(spacing, move |spacing| {
        set_style(list, "gap", (spacing != 0.0).then(|| css::px(spacing)));
    });
    children.mount(host::<ListKind>(list));
    list
}

pub(crate) fn spacer() -> NodeId {
    create("Spacer", "div")
}
