use crate::components::element::{CssValue, ElementEvent};
use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::{Align2, Color, Corners, Extent, Sides, Vec2};

pub(crate) enum Event {}

pub(crate) fn element(
    _tag: &'static str,
    _attributes: Vec<(&'static str, Prop<Option<String>>)>,
    _styles: Vec<(&'static str, Prop<Option<CssValue>>)>,
    _events: Vec<(&'static str, Callback<ElementEvent>)>,
    children: Option<Child>,
) -> NodeId {
    super::frame(
        Prop::Static(Extent::INTRINSIC),
        Prop::Static(Extent::INTRINSIC),
        Prop::Static(None),
        Prop::Static(Sides::ZERO),
        Prop::Static(Align2::STRETCH),
        Prop::Static(Color::TRANSPARENT),
        Prop::Static(Corners::ZERO),
        Prop::Static(None),
        Prop::Static(Vec2::ZERO),
        Prop::Static(0.0),
        Prop::Static(1.0),
        Prop::Static(Sides::ZERO),
        children,
    )
}

pub(crate) fn value(event: &Event) -> Option<String> {
    match *event {}
}

pub(crate) fn key(event: &Event) -> Option<String> {
    match *event {}
}

pub(crate) fn holds(event: &Event, _modifier: &str) -> bool {
    match *event {}
}

pub(crate) fn composing(event: &Event) -> bool {
    match *event {}
}

pub(crate) fn prevent_default(event: &Event) {
    match *event {}
}

pub(crate) fn stop_propagation(event: &Event) {
    match *event {}
}
