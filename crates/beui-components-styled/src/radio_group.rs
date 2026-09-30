use beui_macros::{component, view};

use crate::choice::{self, Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{Callback, Children, Prop};

#[component]
pub fn RadioGroup(
    options: Children<ChoiceOption>,
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    view! {
        <Choice
            options
            selected
            kind=Kind::Radio
            on_change={move |selected| on_change.call(selected)}
        >
            {|handle| view! {
                <OptionFace kind=Kind::Radio handle />
            }}
        </Choice>
    }
}

pub fn radio_group_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}
