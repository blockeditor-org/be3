use std::rc::Rc;

use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    ForEach, Frame, List, Show, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Caption, use_theme};
use block_editor_beui::{ChildBlock, ChildMode, ChildState};
use uuid::Uuid;

use super::state::VideoState;

const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
const STAGE: Color32 = Color32::from_gray(18);

#[component]
pub(crate) fn Player(state: Rc<VideoState>) -> NodeId {
    let shown = Rc::clone(&state);
    let playhead = state.playhead.clone();
    let clips = state.clips.clone();
    let visible = create_memo(clone!(shown playhead clips -> move || {
        let _ = clips.get();
        let _ = playhead.get();
        shown
            .video()
            .map(|video| video.visible_at(playhead.get()))
            .unwrap_or_default()
    }));
    let empty = create_memo(clone!(visible -> move || visible.get().is_empty()));
    let any = create_memo(clone!(empty -> move || !empty.get()));

    let sized = Rc::clone(&state);
    let ratio = create_memo(clone!(visible sized -> move || {
        let ids = visible.get();
        let Some(video) = sized.video() else {
            return DEFAULT_ASPECT_RATIO;
        };
        ids.first()
            .and_then(|id| video.clip(*id))
            .and_then(|clip| sized.target(clip.block_id))
            .and_then(|target| sized.aspect_ratio(target.id))
            .unwrap_or(DEFAULT_ASPECT_RATIO)
    }));
    let theme = use_theme();

    view! {
        <Frame color=STAGE padding_horizontal=8.0 padding_vertical=8.0>
            <List spacing=0.0>
                <Show condition={empty}>
                    <Caption content="No clip at the playhead" color={theme.text_muted.clone()} />
                </Show>
                <Show condition={any}>
                    <Frame aspect_ratio={ratio} color=Color32::BLACK>
                        <List spacing=0.0>
                            <ForEach keys={visible}>
                                {move |id: Uuid| {
                                    let state = Rc::clone(&state);
                                    view! {
                                        <Layer state id />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Layer(state: Rc<VideoState>, id: Uuid) -> NodeId {
    let held = Rc::clone(&state);
    let clips = state.clips.clone();
    let block = create_memo(clone!(held clips -> move || {
        let clip = clips.get().into_iter().find(|clip| clip.id == id)?;
        held.target(clip.block_id)
    }));
    let reported = Rc::clone(&state);
    let watched = block.clone();
    view! {
        <ChildBlock
            editor={state.editor().clone()}
            block={block}
            mode=ChildMode::Preview
            on_state={move |child: ChildState| {
                if let (Some(target), Some(ratio)) =
                    (watched.get_untracked(), child.aspect_ratio)
                {
                    reported.report_aspect_ratio(target.id, ratio);
                }
            }}
        />
    }
}
