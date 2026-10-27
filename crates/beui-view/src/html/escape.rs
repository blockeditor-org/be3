use std::cell::RefCell;

use crate::escape::Escape;
use crate::reactive::{Child, NodeId, Prop};
use crate::types::{Color, FontId};
use beui_core_html::beui_html::Node;
use beui_core_html::css;
use beui_core_html::document::{adopt, element, set_style};

use super::bind;

#[derive(Default)]
pub(crate) struct Inner {
    styles: RefCell<Vec<(&'static str, Prop<Option<String>>)>>,
}

pub(crate) fn embed(_inner: &Inner, child: Child) -> Node {
    element(child).expect("an embedded child is built first")
}

pub(crate) fn style(inner: &Inner, property: &'static str, value: Prop<Option<String>>) {
    inner.styles.borrow_mut().push((property, value));
}

pub(crate) fn color(color: Color) -> String {
    css::color(color)
}

pub(crate) fn font(font: FontId) -> String {
    css::font(font)
}

pub(crate) fn html_escape(build: impl FnOnce(&Escape) -> Node) -> NodeId {
    let escape = Escape(Inner::default());
    let root = build(&escape);
    let node = adopt("Escape", root);
    for (property, value) in escape.0.styles.take() {
        bind(value, move |value| set_style(node, property, value));
    }
    node
}
