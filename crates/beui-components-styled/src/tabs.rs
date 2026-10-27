use beui_macros::{component, view};

use crate::choice::{Collapsing, Kind, OptionFace, Wide};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Tabs(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ChoiceApi>,

    options: Children<ChoiceOption>,
    #[prop(default = 0)] selected: Binding<usize>,
    #[prop(default = String::new())] label: Prop<String>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let choice = ComponentRef::new();
    let built = view! {
        <Collapsing options selected label>|wide: Wide|
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
