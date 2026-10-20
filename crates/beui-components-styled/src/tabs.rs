use beui_macros::{component, view};

use crate::choice::{self, Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Tabs(
    options: Children<ChoiceOption>,
    selected: Prop<usize>,
    on_change: Callback<usize>,
) -> NodeId {
    let options = options.into_run();
    let count = options.clone();
    let selected = Prop::dynamic(move || Some(selected.get().min(count.len().saturating_sub(1))));
    view! {
        <Choice
            options
            selected
            kind=Kind::Tabs
            on_change={move |selected: Option<usize>| {
                if let Some(selected) = selected {
                    on_change.call(selected);
                }
            }}
        >|handle|
            <OptionFace kind=Kind::Tabs handle />
        </Choice>
    }
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    choice::selected_index(document, tabs).unwrap_or(0)
}
