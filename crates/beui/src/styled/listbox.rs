use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Children, Prop};
use crate::styled::choice::{self, Kind, OptionFace};
use crate::unstyled::{Choice, ChoiceOption};

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
