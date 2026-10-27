use crate::components::element::{CssValue, ElementEvent};
use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use beui_core_html::css as format;
use beui_core_html::document::{append, create, element as element_of, set_attribute, set_style};

use super::bind;

pub(crate) type Event = beui_core_html::beui_html::Event;

pub(crate) fn element(
    tag: &'static str,
    attributes: Vec<(&'static str, Prop<Option<String>>)>,
    styles: Vec<(&'static str, Prop<Option<CssValue>>)>,
    events: Vec<(&'static str, Callback<ElementEvent>)>,
    children: Option<Child>,
) -> NodeId {
    let node = create("Element", tag);
    for (name, value) in attributes {
        bind(value, move |value| set_attribute(node, name, value));
    }
    for (property, value) in styles {
        bind(value, move |value| {
            set_style(node, property, value.map(css))
        });
    }
    if let Some(held) = element_of(node) {
        for (name, handler) in events {
            held.listen(
                name,
                Callback::new(move |event: Event| handler.call(ElementEvent(event))),
            );
        }
    }
    if let Some(child) = children {
        append(node, child);
    }
    node
}

fn css(value: CssValue) -> String {
    match value {
        CssValue::Keyword(keyword) => keyword.to_owned(),
        CssValue::Px(length) => format::px(length),
        CssValue::Color(color) => format::color(color),
        CssValue::Font(font) => format::font(font),
        CssValue::Sides(sides) => format::sides(sides),
    }
}

pub(crate) fn value(event: &Event) -> Option<String> {
    event.target_value()
}

pub(crate) fn key(event: &Event) -> Option<String> {
    event.key()
}

pub(crate) fn holds(event: &Event, modifier: &str) -> bool {
    event.holds(modifier)
}

pub(crate) fn composing(event: &Event) -> bool {
    event.composing()
}

pub(crate) fn prevent_default(event: &Event) {
    event.prevent_default();
}

pub(crate) fn stop_propagation(event: &Event) {
    event.stop_propagation();
}
