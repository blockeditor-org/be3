use crate::reactive::Child;
use crate::reactive::Prop;
use crate::reactive::{ChildValue, ComponentContext, NodeId, ScopeContext};
use crate::types::Color;
pub use crate::types::SystemBars;
use beui_macros::component;

#[component]
pub fn Root(
    clear_color: Prop<Color>,
    title: Option<Prop<String>>,
    #[prop(default = SystemBars::default())] system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    crate::core::root(clear_color, title, system_bars, children)
}

pub struct Root {
    child: NodeId,
}

impl ChildValue for Root {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        if component.has_extensions() {
            self.child.finish_component(component, scope);
        }
    }
}

impl Root {
    pub fn child(&self) -> NodeId {
        self.child
    }

    pub fn holding(self, child: NodeId) -> Root {
        Root { child }
    }
}

impl Root {
    pub(crate) fn holding_child(child: NodeId) -> Root {
        Root { child }
    }
}
