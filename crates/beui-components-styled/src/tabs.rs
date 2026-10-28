use beui_macros::{component, view};

use crate::choice::{Collapsing, Kind, OptionFace, Wide};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Tabs<T>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ChoiceApi<T>>,

    options: Children<ChoiceOption<T>>,
    selected: Binding<T>,
    #[prop(default = String::new())] label: Prop<String>,
) -> NodeId
where
    T: Clone + PartialEq + Default + 'static,
{
    let label = create_memo(move || label.get());
    let choice = ComponentRef::new();
    let built = view! {
        <Collapsing options selected label>|wide: Wide<T>|
            <Choice
                api=choice
                options={wide.options}
                selected={wide.selected}
                kind=Kind::Tabs
                spacing={crate::choice::OPTION_SPACING.px()}
                label
            >|handle|
                <OptionFace kind=Kind::Tabs handle />
            </Choice>
        </Collapsing>
    };
    api.fill(choice.get());
    built
}
