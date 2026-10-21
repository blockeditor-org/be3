use crate::reactive::{Child, ChildValue, Get, Prop, create_effect, with_document};
use beui_core::color::Color32;
use beui_core::node::NodeId;
pub use beui_core::window::SystemBars;
use beui_macros::component;

pub struct Root {
    child: NodeId,
}

impl ChildValue for Root {}

impl Root {
    pub fn child(&self) -> NodeId {
        self.child
    }
}

#[component]
pub fn Root(
    clear_color: Prop<Color32>,
    title: Option<Prop<String>>,
    #[prop(default = SystemBars::default())] system_bars: Prop<SystemBars>,
    children: Child,
) -> Root {
    create_effect(move || {
        let color = clear_color.get();
        with_document(|document| {
            document.update_appearance(|appearance| appearance.clear_color = color)
        });
    });
    if let Some(title) = title {
        create_effect(move || {
            let title = title.get();
            with_document(|document| {
                document.update_appearance(|appearance| appearance.title = Some(title))
            });
        });
    }
    create_effect(move || {
        let bars = system_bars.get();
        with_document(|document| {
            document.update_appearance(|appearance| appearance.system_bars = bars)
        });
    });
    Root { child: children }
}
