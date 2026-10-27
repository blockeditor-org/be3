use crate::reactive::Child;
use crate::reactive::ChildValue;
use crate::reactive::ComponentContext;
use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::ScopeContext;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::color::Color;
use beui_core::node::NodeId;
use beui_core::window::SystemBars;

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
    clear_color: Prop<Color>,
    title: Option<Prop<String>>,
    system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    let handle = crate::window::use_window();
    let window = move || {
        handle
            .filter(|handle| handle.is_window())
            .map(|handle| handle.id)
    };
    create_effect(move || {
        let color = crate::colors::resolve_color(clear_color.get());
        let window = window();
        with_document(|document| {
            document.update_appearance(window, |appearance| appearance.clear_color = color)
        });
    });
    if let Some(title) = title {
        create_effect(move || {
            let title = title.get();
            let window = window();
            with_document(|document| {
                document.update_appearance(window, |appearance| appearance.title = Some(title))
            });
        });
    }
    create_effect(move || {
        let bars = system_bars.get();
        let window = window();
        with_document(|document| {
            document.update_appearance(window, |appearance| appearance.system_bars = bars)
        });
    });
    Root { child: children }
}
