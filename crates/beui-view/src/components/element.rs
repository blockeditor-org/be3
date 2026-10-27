use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::{Color, FontId, Sides};
use beui_macros::component;

#[derive(Clone, Debug, PartialEq)]
pub enum CssValue {
    Keyword(&'static str),
    Px(f32),
    Color(Color),
    Font(FontId),
    Sides(Sides),
}

pub struct ElementEvent(pub(crate) crate::core::element::Event);

impl ElementEvent {
    pub fn value(&self) -> Option<String> {
        crate::core::element::value(&self.0)
    }

    pub fn key(&self) -> Option<String> {
        crate::core::element::key(&self.0)
    }

    pub fn holds(&self, modifier: &str) -> bool {
        crate::core::element::holds(&self.0, modifier)
    }

    pub fn composing(&self) -> bool {
        crate::core::element::composing(&self.0)
    }

    pub fn prevent_default(&self) {
        crate::core::element::prevent_default(&self.0);
    }

    pub fn stop_propagation(&self) {
        crate::core::element::stop_propagation(&self.0);
    }
}

#[component]
pub fn Element(
    tag: &'static str,
    #[prop(default = Vec::new())] attributes: Vec<(&'static str, Prop<Option<String>>)>,
    #[prop(default = Vec::new())] styles: Vec<(&'static str, Prop<Option<CssValue>>)>,
    #[prop(default = Vec::new())] events: Vec<(&'static str, Callback<ElementEvent>)>,
    children: Option<Child>,
) -> NodeId {
    crate::core::element::element(tag, attributes, styles, events, children)
}
