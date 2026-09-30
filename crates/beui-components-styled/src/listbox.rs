use beui_macros::{component, view};

use crate::choice::{self, Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{Callback, Children, Prop};

#[component]
pub fn Listbox(
    options: Children<ChoiceOption>,
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    view! {
        <Choice
            options
            selected
            kind=Kind::Listbox
            on_change={move |selected| on_change.call(selected)}
        >
            {|handle| view! {
                <OptionFace kind=Kind::Listbox handle />
            }}
        </Choice>
    }
}

pub fn listbox_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}
