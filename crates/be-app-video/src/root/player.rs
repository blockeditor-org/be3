use be_sdk_beui::beui::Color32;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{ChildBlock, ChildMode};
use uuid::Uuid;

use super::state::VideoState;

const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
const STAGE: Color32 = Color32::from_gray(18);

#[component]
pub(crate) fn Player(state: VideoState) -> NodeId {
    let playhead = state.playhead;
    let edit = state.edit;
    let visible = create_memo(move || {
        let playhead = playhead.get();
        edit.with(|video| video.visible_at(playhead))
    });
    let empty = create_memo(move || visible.get().is_empty());

    view! {
        <Frame fill={ColorRole::Custom(STAGE.into())} padding=Space::Md>
            <List spacing=Space::None>
                <Show
                    value={empty}
                    fallback={move || view! {
                        <Frame
                            aspect_ratio=DEFAULT_ASPECT_RATIO
                            fill={ColorRole::Custom(Color32::BLACK.into())}
                        >
                            <List spacing=Space::None>
                                <ForEach value={visible}>|id: Uuid|
                                    <Layer state id />
                                </ForEach>
                            </List>
                        </Frame>
                    }}
                >
                    <Text style=TextStyle::CAPTION>"No clip at the playhead"</Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Layer(state: VideoState, id: Uuid) -> NodeId {
    let clips = state.clips;
    let block = create_memo(move || {
        let clip = clips.get().into_iter().find(|clip| clip.id == id)?;
        state.target(clip.block_id)
    });
    view! {
        <ChildBlock editor={state.editor()} block={block} mode=ChildMode::Preview />
    }
}
