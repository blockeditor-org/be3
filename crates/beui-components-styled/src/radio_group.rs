use beui_macros::{component, view};

use crate::choice::{Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn RadioGroup(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ChoiceApi>,

    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
) -> NodeId {
    view! {
        <Choice
            api
            options
            selected
            kind=Kind::Radio
            spacing={crate::choice::OPTION_SPACING.px()}
        >|handle|
            <OptionFace kind=Kind::Radio handle />
        </Choice>
    }
}
