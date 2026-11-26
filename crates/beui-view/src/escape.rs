use crate::reactive::{Child, IntoPropValue, NodeId, Prop};
use crate::types::{Color, FontId};

pub use beui_html;

pub struct Escape(pub(crate) crate::core::escape::Inner);

impl Escape {
    pub fn embed(&self, child: Child) -> beui_html::Node {
        crate::core::escape::embed(&self.0, child)
    }

    pub fn style<M1>(
        &self,
        property: &'static str,
        value: impl IntoPropValue<Prop<Option<String>>, M1>,
    ) {
        crate::core::escape::style(&self.0, property, value.into_prop_value());
    }

    pub fn color(color: Color) -> String {
        crate::core::escape::color(color)
    }

    pub fn font(font: FontId) -> String {
        crate::core::escape::font(font)
    }
}

pub fn html_escape(build: impl FnOnce(&Escape) -> beui_html::Node) -> NodeId {
    crate::core::escape::html_escape(build)
}
