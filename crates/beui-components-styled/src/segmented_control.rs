use beui_macros::{component, view};

use crate::choice::{Collapsing, Kind, SegmentFace, Wide};
use crate::frame::{Border, Frame, Radius};
use beui_components_unstyled::{Choice, ChoiceOption};
use beui_core::base::{Direction, Sides};
use beui_core::node::NodeId;
use beui_view::primitives::List;
use beui_view::reactive::*;

const TRACK_PADDING: f32 = 2.0;
const SEGMENT_SPACING: f32 = 2.0;

#[component]
pub fn SegmentedControl(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ChoiceApi>,

    options: Children<ChoiceOption>,
    #[prop(default = 0)] selected: Binding<usize>,
    #[prop(default = String::new())] label: Prop<String>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let choice = ComponentRef::new();
    let built = view! {
        <Collapsing options selected label>|wide: Wide|
            <List direction=Direction::Horizontal spacing=0.0>
            <Frame border=Border::DEFAULT radius=Radius::Md padding={Sides::all(TRACK_PADDING)}>
                <Choice
                    api=choice
                    options={wide.options}
                    selected={wide.selected}
                    kind=Kind::Radio
                    direction=Direction::Horizontal
                    spacing=SEGMENT_SPACING
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
