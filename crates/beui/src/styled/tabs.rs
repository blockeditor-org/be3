use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Children, Prop};
use crate::styled::choice::{self, Kind, OptionFace};
use crate::unstyled::{Choice, ChoiceOption};

#[component]
pub fn Tabs(
    options: Children<ChoiceOption>,
    selected: Prop<usize>,
    on_change: Callback<usize>,
) -> NodeId {
    let options = options.into_run();
    let count = options.clone();
    let selected = selected.map(move |selected| Some(selected.min(count.len().saturating_sub(1))));
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
        >
            {|handle| view! {
                <OptionFace kind=Kind::Tabs handle />
            }}
        </Choice>
    }
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    choice::selected_index(document, tabs).unwrap_or(0)
}
