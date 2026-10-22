use beui_macros::{component, view};

use crate::choice::{self, Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Tabs(
    options: Children<ChoiceOption>,
    #[prop(default = 0)] selected: Binding<usize>,
) -> NodeId {
    let options = options.into_run();
    let count = options.clone();
    let shown = Prop::dynamic(move || Some(selected.get().min(count.len().saturating_sub(1))));
    let chosen = Binding::new(shown, move |chosen: Option<usize>| {
        if let Some(chosen) = chosen {
            selected.set(chosen);
        }
    });
    view! {
        <Choice options selected={chosen} kind=Kind::Tabs>|handle|
            <OptionFace kind=Kind::Tabs handle />
        </Choice>
    }
}

pub fn tabs_selected(tree: &impl Inspect, tabs: NodeId) -> usize {
    choice::selected_index(tree, tabs).unwrap_or(0)
}
