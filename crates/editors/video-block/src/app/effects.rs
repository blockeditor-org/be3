
use block_editor_beui::be_block::video::{
    MAX_CLIP_LENGTH, VideoAttachment, VideoClip, VideoEffect,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_LINK_OFF;
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, List, Memo, Show, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, Heading, IconButton, NumberInput, Separator, use_theme,
};

use crate::timeline::timecode;
use uuid::Uuid;

use super::state::VideoState;

#[component]
pub(crate) fn EffectsPanel(state: VideoState) -> NodeId {
    let clip = create_memo(move || state.selected_clip());
    let chosen = create_memo(move || clip.get().is_some());
    let none = create_memo(move || !chosen.get());
    let theme = use_theme();
    view! {
        <List spacing=8.0>
            <Heading content="Effects" />
            <Separator />
            <Show condition={none}>
                <Caption content="Select a clip to see its effects." color={theme.text_muted} />
            </Show>
            <Show condition={chosen}>
                <ClipInspector state={state} clip={clip} />
            </Show>
        </List>
    }
}

#[component]
fn ClipInspector(state: VideoState, clip: Memo<Option<VideoClip>>) -> NodeId {
    let name = create_memo(move || {
        clip.get().map_or_else(String::new, |clip| state.name_of(clip.block_id))
    });
    let length = create_memo(move || clip.get().map_or(1.0, |clip| clip.length as f64));
    let rate = state.frame_rate;
    let seconds = create_memo(move || {
        let frames = clip.get().map_or(0, |clip| clip.length);
        format!("{:.2}s", rate.get().seconds(frames))
    });
    let starts = create_memo(move || {
        let Some(clip) = clip.get() else {
            return String::new();
        };
        let start = state
            .video()
            .and_then(|video| video.timing(clip.id).map(|timing| timing.start))
            .unwrap_or(0);
        timecode(rate.get(), start)
    });
    let attached = create_memo(move || clip.get().and_then(|clip| clip.attachment));
    let hanging = create_memo(move || attached.get().is_some());
    let parent_name = create_memo(move || {
        let Some(attachment) = attached.get() else {
            return "Base track".to_owned();
        };
        state
            .video()
            .and_then(|video| video.clip(attachment.clip_id).cloned())
            .map_or_else(|| "Loading…".to_owned(), |clip| state.name_of(clip.block_id))
    });
    let offset = create_memo(move || {
        attached
            .get()
            .map_or(0.0, |attachment| attachment.offset as f64)
    });

    let trimmed_clip = clip;
    let detached_clip = clip;
    let moved_clip = clip;
    let effects = create_memo(move || clip.get().map(|clip| clip.effects).unwrap_or_default());
    let bare = create_memo(move || effects.get().is_empty());
    let effect_ids = create_memo(move || {
        effects
            .get()
            .iter()
            .map(|effect| effect.id)
            .collect::<Vec<Uuid>>()
    });
    let theme = use_theme();
    let seconds_color = theme.text_muted;
    let start_color = theme.text_muted;
    let parent_color = theme.text_muted;
    let bare_color = theme.text_muted;
    let hanging_row = hanging;

    view! {
        <List spacing=8.0>
            <Body content={name} />
            <NumberInput
                value={length}
                min=1.0
                max={MAX_CLIP_LENGTH as f64}
                label="Length"
                @test_id={"video.clip-length"}
                on_change={move |value: f64| {
                    let Some(mut clip) = trimmed_clip.get_untracked() else {
                        return;
                    };
                    clip.length = (value.round().max(1.0) as u64).min(MAX_CLIP_LENGTH);
                    state.update_clip(clip);
                }}
            />
            <Caption content={seconds} color={seconds_color} />
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Body content="Starts at" />
                <Caption content={starts} color={start_color} />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Body content="Attached to" />
                <Caption content={parent_name} color={parent_color} />
                <Show condition={hanging_row}>
                    {move || view! {
                        <IconButton
                            glyph={ICON_LINK_OFF.to_owned()}
                            label="Move this clip onto the base track"
                            @test_id={"video.detach"}
                            on_click={move || {
                                let Some(mut clip) = detached_clip.get_untracked() else {
                                    return;
                                };
                                clip.attachment = None;
                                state.update_clip(clip);
                            }}
                        />
                    }}
                </Show>
            </List>
            <Show condition={hanging}>
                {move || view! {
                    <NumberInput
                        value={offset}
                        label="Offset"
                        on_change={move |value: f64| {
                            let Some(mut clip) = moved_clip.get_untracked() else {
                                return;
                            };
                            let Some(attachment) = clip.attachment else {
                                return;
                            };
                            clip.attachment =
                                Some(VideoAttachment::new(attachment.clip_id, value.round() as i64));
                            state.update_clip(clip);
                        }}
                    />
                }}
            </Show>
            <Body content="Effect stack" />
            <Show condition={bare}>
                <Caption content="No effects yet." color={bare_color} />
            </Show>
            <ForEach keys={effect_ids}>
                {move |id: Uuid| {
                    let effects = effects;
                    view! {
                        <EffectRow effects id />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn EffectRow(effects: Memo<Vec<VideoEffect>>, id: Uuid) -> NodeId {
    let held = create_memo(move || effects.get().into_iter().find(|effect| effect.id == id));
    let state = create_memo(
        move || match held.get().is_some_and(|effect| effect.enabled) {
            true => "On".to_owned(),
            false => "Off".to_owned(),
        },
    );
    let name = create_memo(move || held.get().map(|effect| effect.name).unwrap_or_default());
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <Caption content={state} color={theme.text_muted} />
            <Body content={name} />
        </List>
    }
}
