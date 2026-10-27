use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_UNLINK;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use video_block_types::{MAX_CLIP_LENGTH, VideoAttachment, VideoClip, VideoEffect};

use crate::timeline::timecode;
use uuid::Uuid;

use super::state::VideoState;
use beui::{Align, Direction};

#[component]
pub(crate) fn EffectsPanel(state: VideoState) -> NodeId {
    let clip = create_memo(move || state.selected_clip());
    let chosen = create_memo(move || clip.get().is_some());
    let none = create_memo(move || !chosen.get());
    view! {
        <List spacing=Space::Md>
            <Text style=TextStyle::HEADING>"Effects"</Text>
            <Separator />
            <Show condition={none}>
                <Text style=TextStyle::CAPTION>"Select a clip to see its effects."</Text>
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
        clip.get()
            .map_or_else(String::new, |clip| state.name_of(clip.block_id))
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
            .map_or_else(
                || "Loading…".to_owned(),
                |clip| state.name_of(clip.block_id),
            )
    });
    let offset = create_memo(move || {
        attached
            .get()
            .map_or(0.0, |attachment| attachment.offset as f64)
    });

    let effects = create_memo(move || clip.get().map(|clip| clip.effects).unwrap_or_default());
    let bare = create_memo(move || effects.get().is_empty());
    let effect_ids = create_memo(move || {
        effects
            .get()
            .iter()
            .map(|effect| effect.id)
            .collect::<Vec<Uuid>>()
    });
    let seconds_color = TextColor::Muted;
    let start_color = TextColor::Muted;
    let parent_color = TextColor::Muted;
    let bare_color = TextColor::Muted;

    view! {
        <List spacing=Space::Md>
            <Text>{name}</Text>
            <NumberInput
                value={Binding::new(length, move |value: f64| {
                    let Some(mut clip) = clip.get_untracked() else {
                        return;
                    };
                    clip.length = (value.round().max(1.0) as u64).min(MAX_CLIP_LENGTH);
                    state.update_clip(clip);
                })}
                min=1.0
                max={MAX_CLIP_LENGTH as f64}
                label="Length"
                @test_id={"video.clip-length"}
            />
            <Text style=TextStyle::CAPTION color={seconds_color}>{seconds}</Text>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Text>"Starts at"</Text>
                <Text style=TextStyle::CAPTION color={start_color}>{starts}</Text>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Text>"Attached to"</Text>
                <Text style=TextStyle::CAPTION color={parent_color}>{parent_name}</Text>
                <Show condition={hanging}>
                    <Button
                        icon_only=true
                        glyph=ICON_UNLINK
                        label="Move this clip onto the base track"
                        @test_id={"video.detach"}
                        on_click={move || {
                            let Some(mut clip) = clip.get_untracked() else {
                                return;
                            };
                            clip.attachment = None;
                            state.update_clip(clip);
                        }}
                    />
                </Show>
            </List>
            <Show condition={hanging}>
                <NumberInput
                    value={Binding::new(offset, move |value: f64| {
                        let Some(mut clip) = clip.get_untracked() else {
                            return;
                        };
                        let Some(attachment) = clip.attachment else {
                            return;
                        };
                        clip.attachment =
                            Some(VideoAttachment::new(attachment.clip_id, value.round() as i64));
                        state.update_clip(clip);
                    })}
                    label="Offset"
                />
            </Show>
            <Text>"Effect stack"</Text>
            <Show condition={bare}>
                <Text style=TextStyle::CAPTION color={bare_color}>"No effects yet."</Text>
            </Show>
            <ForEach keys={effect_ids}>|id: Uuid|
                <EffectRow effects id />
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
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
            <Text style=TextStyle::CAPTION>{state}</Text>
            <Text>{name}</Text>
        </List>
    }
}
