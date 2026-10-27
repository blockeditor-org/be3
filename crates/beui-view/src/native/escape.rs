use crate::escape::Escape;
use crate::reactive::{Child, NodeId, Prop};
use crate::types::{Color, FontId};
use beui_html::Node;

pub(crate) enum Inner {}

const HTML_ONLY: &str = "html_escape builds the HTML core's own nodes: call it only when CORE is Core::Html";

pub(crate) fn embed(inner: &Inner, _child: Child) -> Node {
    match *inner {}
}

pub(crate) fn style(inner: &Inner, _property: &'static str, _value: Prop<Option<String>>) {
    match *inner {}
}

pub(crate) fn color(_color: Color) -> String {
    unreachable!("{HTML_ONLY}")
}

pub(crate) fn font(_font: FontId) -> String {
    unreachable!("{HTML_ONLY}")
}

pub(crate) fn html_escape(_build: impl FnOnce(&Escape) -> Node) -> NodeId {
    unreachable!("{HTML_ONLY}")
}
