use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::ICON_UNLINK;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_type_video::{MAX_CLIP_LENGTH, VideoAttachment, VideoClip, VideoEffect};

use crate::timeline::timecode;

use super::state::VideoState;
use beui::{Align, Direction};

#[component]
pub(crate) fn EffectsPanel(state: VideoState) -> NodeId {
    view! {
        <List spacing=Space::Md>
            <Show
                value={move || state.selected_clip()}
                fallback={|| view! {
                    <Text style=TextStyle::CAPTION>"Select a clip to see its effects."</Text>
                }}
            >|clip: ReadSignal<VideoClip>|
                <ClipInspector state={state} clip={clip} />
            </Show>
        </List>
    }
}

#[component]
fn ClipInspector(state: VideoState, clip: ReadSignal<VideoClip>) -> NodeId {
    let name = create_memo(move || state.name_of(clip.with(|clip| clip.block_id)));
    let length = create_memo(move || clip.with(|clip| clip.length as f64));
    let rate = state.frame_rate;
    let seconds = create_memo(move || {
        let frames = clip.with(|clip| clip.length);
        format!("{:.2}s", rate.get().seconds(frames))
    });
    let starts = create_memo(move || {
        let clip = clip.get();
        let start = state
            .video()
            .and_then(|video| video.timing(clip.id).map(|timing| timing.start))
            .unwrap_or(0);
        timecode(rate.get(), start)
    });
    let attached = create_memo(move || clip.with(|clip| clip.attachment));
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

    let seconds_color = TextColor::Muted;
    let start_color = TextColor::Muted;
    let parent_color = TextColor::Muted;
    let bare_color = TextColor::Muted;

    view! {
        <List spacing=Space::Md>
            <Text>{name}</Text>
            <NumberInput
                value={Binding::new(length, move |value: f64| {
                    let mut clip = clip.get_untracked();
                    clip.length = (value.round().max(1.0) as u64).min(MAX_CLIP_LENGTH);
                    state.update_clip(clip);
                })}
                min=1.0
                max={MAX_CLIP_LENGTH as f64}
                label="Length"
                on_scrub={move |change| state.drag_value(change)}
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
                <Show value={hanging}>
                    <Button
                        icon_only=true
                        glyph=ICON_UNLINK
                        label="Move this clip onto the base track"
                        @test_id={"video.detach"}
                        on_click={move || {
                            let mut clip = clip.get_untracked();
                            clip.attachment = None;
                            state.update_clip(clip);
                        }}
                    />
                </Show>
            </List>
            <Show value={hanging}>
                <NumberInput
                    value={Binding::new(offset, move |value: f64| {
                        let mut clip = clip.get_untracked();
                        let Some(attachment) = clip.attachment else {
                            return;
                        };
                        clip.attachment =
                            Some(VideoAttachment::new(attachment.clip_id, value.round() as i64));
                        state.update_clip(clip);
                    })}
                    label="Offset"
                    on_scrub={move |change| state.drag_value(change)}
                />
            </Show>
            <Text>"Effect stack"</Text>
            <ForEach
                value={move || clip.with(|clip| clip.effects.clone())}
                key={|effect: &VideoEffect| effect.id}
                fallback={move || view! {
                    <Text style=TextStyle::CAPTION color={bare_color}>"No effects yet."</Text>
                }}
            >|effect: ReadSignal<VideoEffect>|
                <EffectRow effect />
            </ForEach>
        </List>
    }
}

#[component]
fn EffectRow(effect: ReadSignal<VideoEffect>) -> NodeId {
    let state = create_memo(move || match effect.with(|effect| effect.enabled) {
        true => "On".to_owned(),
        false => "Off".to_owned(),
    });
    let name = create_memo(move || effect.with(|effect| effect.name.clone()));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
            <Text style=TextStyle::CAPTION>{state}</Text>
            <Text>{name}</Text>
        </List>
    }
}
