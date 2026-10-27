use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{ChildBlock, ChildMode, ChildState};
use uuid::Uuid;

use super::state::VideoState;

const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
const STAGE: Color32 = Color32::from_gray(18);

#[component]
pub(crate) fn Player(state: VideoState) -> NodeId {
    let playhead = state.playhead;
    let clips = state.clips;
    let visible = create_memo(move || {
        let _ = clips.get();
        let _ = playhead.get();
        state
            .video()
            .map(|video| video.visible_at(playhead.get()))
            .unwrap_or_default()
    });
    let empty = create_memo(move || visible.get().is_empty());
    let any = create_memo(move || !empty.get());

    let ratio = create_memo(move || {
        let ids = visible.get();
        let Some(video) = state.video() else {
            return DEFAULT_ASPECT_RATIO;
        };
        ids.first()
            .and_then(|id| video.clip(*id))
            .and_then(|clip| state.target(clip.block_id))
            .and_then(|target| state.aspect_ratio(target.id))
            .unwrap_or(DEFAULT_ASPECT_RATIO)
    });

    view! {
        <Frame fill={ColorRole::Custom(STAGE.into())} padding=Space::Md>
            <List spacing=Space::None>
                <Show condition={empty}>
                    <Text style=TextStyle::CAPTION>"No clip at the playhead"</Text>
                </Show>
                <Show condition={any}>
                    <Frame aspect_ratio={ratio} fill={ColorRole::Custom(Color32::BLACK.into())}>
                        <List spacing=Space::None>
                            <ForEach keys={visible}>|id: Uuid|
                                <Layer state id />
                            </ForEach>
                        </List>
                    </Frame>
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
        <ChildBlock
            editor={state.editor()}
            block={block}
            mode=ChildMode::Preview
            on_state={move |child: ChildState| {
                if let (Some(target), Some(ratio)) =
                    (block.get_untracked(), child.aspect_ratio)
                {
                    state.report_aspect_ratio(target.id, ratio);
                }
            }}
        />
    }
}
