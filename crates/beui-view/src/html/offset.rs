use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Direction;
use crate::types::PlatformScroll;
use crate::types::ScrollPosition;
use beui_core_html::css;
use beui_core_html::document::{append, create, set_placed_style, set_style};

use super::bind;

pub(crate) fn offset(
    offset: Prop<f32>,
    direction: Prop<Direction>,
    fit: bool,
    follow_end: Prop<bool>,
    platform_scroll: PlatformScroll,
    on_change: Callback<ScrollPosition>,
    children: Child,
) -> NodeId {
    let node = create("Offset", "div");
    set_style(node, "display", Some("flex".to_owned()));
    if platform_scroll == PlatformScroll::Off {
        set_style(node, "overflow", Some("hidden".to_owned()));
    }
    set_style(node, "overflow-anchor", Some("none".to_owned()));
    if platform_scroll == PlatformScroll::Bare {
        set_style(node, "scrollbar-width", Some("none".to_owned()));
    }
    append(node, children);
    set_placed_style(children, "flex-shrink", Some("0".to_owned()));
    let scroller = beui_core_html::scroll::scroller(node, children, on_change);
    let held = scroller.clone();
    bind(direction, move |direction| {
        held.set_direction(direction);
        set_style(
            node,
            "flex-direction",
            Some(css::flex_direction(direction).to_owned()),
        );
        if platform_scroll != PlatformScroll::Off {
            let (along, across) = match direction {
                Direction::Horizontal => ("overflow-x", "overflow-y"),
                Direction::Vertical => ("overflow-y", "overflow-x"),
            };
            set_style(node, along, Some("auto".to_owned()));
            set_style(node, across, Some("hidden".to_owned()));
        }
        let (fill, other) = match direction {
            Direction::Horizontal => ("min-width", "min-height"),
            Direction::Vertical => ("min-height", "min-width"),
        };
        set_placed_style(children, fill, fit.then(|| "100%".to_owned()));
        set_placed_style(children, other, None);
    });
    let held = scroller.clone();
    bind(follow_end, move |follows| held.set_follows_end(follows));
    bind(offset, move |offset| scroller.set_offset(offset));
    node
}
