use std::rc::Rc;

use block_editor_beui::be_block::video::{
    MAX_CLIP_LENGTH, VideoAttachment, VideoClip, VideoEffect,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_LINK_OFF;
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, List, Memo, Show, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, Heading, IconButton, NumberInput, Separator, use_theme,
};

use crate::timeline::timecode;
use uuid::Uuid;

use super::state::VideoState;

#[component]
pub(crate) fn EffectsPanel(state: Rc<VideoState>) -> NodeId {
    let held = Rc::clone(&state);
    let clip = create_memo(clone!(held -> move || held.selected_clip()));
    let chosen = create_memo(clone!(clip -> move || clip.get().is_some()));
    let none = create_memo(clone!(chosen -> move || !chosen.get()));
    let theme = use_theme();
    view! {
        <List spacing=8.0>
            <Heading content="Effects" />
            <Separator />
            <Show condition={none}>
                <Caption
                    content="Select a clip to see its effects."
                    color={theme.text_muted.clone()}
                />
            </Show>
            <Show condition={chosen}>
                <ClipInspector state={state} clip={clip} />
            </Show>
        </List>
    }
}

#[component]
fn ClipInspector(state: Rc<VideoState>, clip: Memo<Option<VideoClip>>) -> NodeId {
    let named = Rc::clone(&state);
    let name = create_memo(clone!(clip named -> move || {
        clip.get().map_or_else(String::new, |clip| named.name_of(clip.block_id))
    }));
    let length =
        create_memo(clone!(clip -> move || clip.get().map_or(1.0, |clip| clip.length as f64)));
    let rate = state.frame_rate.clone();
    let seconds = create_memo(clone!(clip rate -> move || {
        let frames = clip.get().map_or(0, |clip| clip.length);
        format!("{:.2}s", rate.get().seconds(frames))
    }));
    let timed = Rc::clone(&state);
    let starts = create_memo(clone!(clip timed rate -> move || {
        let Some(clip) = clip.get() else {
            return String::new();
        };
        let start = timed
            .video()
            .and_then(|video| video.timing(clip.id).map(|timing| timing.start))
            .unwrap_or(0);
        timecode(rate.get(), start)
    }));
    let attached = create_memo(clone!(clip -> move || clip.get().and_then(|clip| clip.attachment)));
    let hanging = create_memo(clone!(attached -> move || attached.get().is_some()));
    let parent = Rc::clone(&state);
    let parent_name = create_memo(clone!(attached parent -> move || {
        let Some(attachment) = attached.get() else {
            return "Base track".to_owned();
        };
        parent
            .video()
            .and_then(|video| video.clip(attachment.clip_id).cloned())
            .map_or_else(|| "Loading…".to_owned(), |clip| parent.name_of(clip.block_id))
    }));
    let offset = create_memo(clone!(attached -> move || {
        attached.get().map_or(0.0, |attachment| attachment.offset as f64)
    }));

    let trimmed = Rc::clone(&state);
    let trimmed_clip = clip.clone();
    let detached = Rc::clone(&state);
    let detached_clip = clip.clone();
    let moved = Rc::clone(&state);
    let moved_clip = clip.clone();
    let effects = create_memo(clone!(clip -> move || {
        clip.get().map(|clip| clip.effects).unwrap_or_default()
    }));
    let bare = create_memo(clone!(effects -> move || effects.get().is_empty()));
    let effect_ids = create_memo(clone!(effects -> move || {
        effects.get().iter().map(|effect| effect.id).collect::<Vec<Uuid>>()
    }));
    let theme = use_theme();
    let seconds_color = theme.text_muted.clone();
    let start_color = theme.text_muted.clone();
    let parent_color = theme.text_muted.clone();
    let bare_color = theme.text_muted.clone();
    let hanging_row = hanging.clone();

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
                    trimmed.update_clip(clip);
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
                    <IconButton
                        glyph={ICON_LINK_OFF.to_owned()}
                        label="Move this clip onto the base track"
                        @test_id={"video.detach"}
                        on_click={move || {
                            let Some(mut clip) = detached_clip.get_untracked() else {
                                return;
                            };
                            clip.attachment = None;
                            detached.update_clip(clip);
                        }}
                    />
                </Show>
            </List>
            <Show condition={hanging}>
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
                        moved.update_clip(clip);
                    }}
                />
            </Show>
            <Body content="Effect stack" />
            <Show condition={bare}>
                <Caption content="No effects yet." color={bare_color} />
            </Show>
            <ForEach keys={effect_ids}>
                {move |id: Uuid| {
                    let effects = effects.clone();
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
    let held = create_memo(clone!(effects -> move || {
        effects.get().into_iter().find(|effect| effect.id == id)
    }));
    let state = create_memo(clone!(held -> move || {
        match held.get().is_some_and(|effect| effect.enabled) {
            true => "On".to_owned(),
            false => "Off".to_owned(),
        }
    }));
    let name = create_memo(clone!(held -> move || {
        held.get().map(|effect| effect.name).unwrap_or_default()
    }));
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <Caption content={state} color={theme.text_muted.clone()} />
            <Body content={name} />
        </List>
    }
}
