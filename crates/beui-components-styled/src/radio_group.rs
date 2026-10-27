use beui_macros::{component, view};

use crate::choice::{Kind, OptionFace};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn RadioGroup<T>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ChoiceApi<T>>,

    options: Children<ChoiceOption<T>>,
    #[prop(default = None)] selected: Binding<Option<T>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
{
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
