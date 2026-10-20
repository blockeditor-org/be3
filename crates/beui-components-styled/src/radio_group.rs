use beui_macros::{component, view};

use crate::choice::{self, Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn RadioGroup(
    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
) -> NodeId {
    view! {
        <Choice options selected kind=Kind::Radio>|handle|
            <OptionFace kind=Kind::Radio handle />
        </Choice>
    }
}

pub fn radio_group_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}
