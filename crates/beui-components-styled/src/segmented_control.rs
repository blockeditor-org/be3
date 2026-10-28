use beui_macros::{component, view};

use crate::choice::{Collapsing, Kind, SegmentFace, Wide};
use crate::frame::{Border, Frame, Radius};
use crate::list::List;
use crate::tokens::{Space, use_space};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;

const TRACK_PADDING: Space = Space::Xs2;
const SEGMENT_SPACING: Space = Space::Xs2;

#[component]
pub fn SegmentedControl<T>(
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
            <List direction=Direction::Horizontal spacing=Space::None>
                <Frame border=Border::DEFAULT radius=Radius::Md padding=TRACK_PADDING>
                    <Choice
                        api=choice
                        options={wide.options}
                        selected={wide.selected}
                        kind=Kind::Radio
                        direction=Direction::Horizontal
                        spacing={use_space(SEGMENT_SPACING).get_untracked()}
                        label
                    >|handle|
                        <SegmentFace handle />
                    </Choice>
                </Frame>
            </List>
        </Collapsing>
    };
    api.fill(choice.get());
    built
}
