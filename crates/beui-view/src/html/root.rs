use crate::reactive::Child;
use crate::reactive::ChildValue;
use crate::reactive::ComponentContext;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ScopeContext;
use crate::reactive::create_effect;
use crate::types::Color32;
use crate::types::SystemBars;

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

pub fn root(
    clear_color: Prop<Color32>,
    title: Option<Prop<String>>,
    system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    let _ = system_bars;
    create_effect(move || beui_core_html::run::set_background(clear_color.get()));
    if let Some(title) = title {
        create_effect(move || beui_core_html::page::set_title(&title.get()));
    }
    Root { child: children }
}
