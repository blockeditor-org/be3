use block_editor_plugin::be_block::VideoContent;
use std::rc::Rc;

use block_editor_plugin::be_block::video::{VideoFrameRate, VideoOperation};
use block_editor_plugin::beui::icons::{
    ICON_ADD, ICON_CONTENT_CUT, ICON_DELETE, ICON_FIT_SCREEN, ICON_PAUSE, ICON_PLAY_ARROW,
    ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS, ICON_SUBDIRECTORY_ARROW_RIGHT, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Prop, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Caption, IconButton, Scroll, Select, Separator, use_theme,
};
use block_editor_plugin::beui::unstyled::ChoiceOption;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor, Toolbar};
use uuid::Uuid;

pub(crate) mod effects;
pub(crate) mod player;
pub(crate) mod state;
pub(crate) mod timeline;

use effects::EffectsPanel;
use player::Player;
use state::{VideoState, ZOOM_STEP};
use timeline::Timeline;

const DEFAULT_EDITOR_SIZE: Vec2 = Vec2::new(1000.0, 640.0);
const EFFECTS_WIDTH: f32 = 268.0;
const TIMELINE_SHARE: f32 = 42.0;
const TOOL_SEPARATOR_LENGTH: f32 = 20.0;

const FRAME_RATES: [VideoFrameRate; 7] = [
    VideoFrameRate::new(24, 1),
    VideoFrameRate::new(24_000, 1001),
    VideoFrameRate::new(25, 1),
    VideoFrameRate::new(30, 1),
    VideoFrameRate::new(30_000, 1001),
    VideoFrameRate::new(50, 1),
    VideoFrameRate::new(60, 1),
];

pub struct VideoApp;

impl block_editor_plugin::BeuiApp for VideoApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <VideoEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <VideoPreview editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&VideoContent::default()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(DEFAULT_EDITOR_SIZE)
    }
}

#[component]
fn VideoEditor(editor: Editor) -> NodeId {
    let state = VideoState::new(&editor);
    let polled = Rc::clone(&state);
    editor.each_frame(move || polled.poll());

    let chrome = editor.chrome_shown();
    let bar = Rc::clone(&state);
    let effects = Rc::clone(&state);
    let player = Rc::clone(&state);
    let playback = Rc::clone(&state);
    let tools = Rc::clone(&state);

    view! {
        <List spacing=0.0>
            <FrameRateBar state={bar} shown={chrome} />
            <List
                @sizing=ItemSize::Percent(100.0 - TIMELINE_SHARE)
                direction=Direction::Horizontal
                spacing=0.0
            >
                <EffectsColumn state={effects} />
                <Separator direction=Direction::Vertical />
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <Player @sizing=ItemSize::Percent(100.0) state={player} />
                    <Separator />
                    <PlaybackBar state={playback} />
                </List>
            </List>
            <Separator />
            <TimelineTools state={tools} />
            <Separator />
            <Timeline @sizing=ItemSize::Percent(TIMELINE_SHARE) state={state} />
        </List>
    }
}

#[component]
fn VideoPreview(editor: Editor) -> NodeId {
    let state = VideoState::new(&editor);
    let polled = Rc::clone(&state);
    editor.each_frame(move || polled.poll());
    view! {
        <Player state={state} />
    }
}

#[component]
fn EffectsColumn(state: Rc<VideoState>) -> NodeId {
    view! {
        <Frame width=EFFECTS_WIDTH padding_horizontal=10.0 padding_vertical=10.0>
            <Scroll>
                <EffectsPanel state={state} />
            </Scroll>
        </Frame>
    }
}

#[component]
fn FrameRateBar(state: Rc<VideoState>, shown: Prop<bool>) -> NodeId {
    let rate = state.frame_rate.clone();
    let chosen = create_memo(clone!(rate -> move || {
        FRAME_RATES.into_iter().position(|option| option == rate.get())
    }));
    let options = (0..FRAME_RATES.len()).collect::<Vec<usize>>();
    view! {
        <Toolbar shown={shown}>
            <Select
                options={view! {
                    <ForEach keys={options}>
                        {|index: usize| view! {
                            <ChoiceOption label={frame_rate_label(FRAME_RATES[index])} />
                        }}
                    </ForEach>
                }}
                selected={chosen}
                label="Frames per second"
                @test_id={"video.frame-rate"}
                on_change={move |index: Option<usize>| {
                    let Some(frame_rate) = index.and_then(|index| FRAME_RATES.get(index).copied())
                    else {
                        return;
                    };
                    state.operate(VideoOperation::SetFrameRate { frame_rate });
                }}
            />
        </Toolbar>
    }
}

#[component]
fn PlaybackBar(state: Rc<VideoState>) -> NodeId {
    let start = Rc::clone(&state);
    let play = Rc::clone(&state);
    let end = Rc::clone(&state);
    let playing = state.playing.clone();
    let empty = create_memo(clone!(state -> move || state.duration.get() == 0));
    let glyph = create_memo(clone!(playing -> move || match playing.get() {
        true => ICON_PAUSE.to_owned(),
        false => ICON_PLAY_ARROW.to_owned(),
    }));
    let playhead = state.playhead.clone();
    let duration = state.duration.clone();
    let rate = state.frame_rate.clone();
    let position = create_memo(clone!(playhead duration rate -> move || {
        format!(
            "{} / {}",
            crate::timeline::timecode(rate.get(), playhead.get()),
            crate::timeline::timecode(rate.get(), duration.get())
        )
    }));
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=10.0 padding_vertical=4.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_SKIP_PREVIOUS.to_owned()}
                    label="Go to the start"
                    on_click={move || start.seek(0)}
                />
                <IconButton
                    glyph={glyph}
                    label="Play"
                    disabled={empty}
                    @test_id={"video.play"}
                    on_click={move || play.toggle_playback()}
                />
                <IconButton
                    glyph={ICON_SKIP_NEXT.to_owned()}
                    label="Go to the end"
                    on_click={move || end.seek(end.duration.get_untracked())}
                />
                <Caption content={position} color={theme.text_muted.clone()} />
            </List>
        </Frame>
    }
}

#[component]
fn TimelineTools(state: Rc<VideoState>) -> NodeId {
    let add = Rc::clone(&state);
    let attach = Rc::clone(&state);
    let split = Rc::clone(&state);
    let remove = Rc::clone(&state);
    let out = Rc::clone(&state);
    let inward = Rc::clone(&state);
    let fit = Rc::clone(&state);
    let none = create_memo(clone!(state -> move || state.selected.get().is_none()));
    view! {
        <Frame padding_horizontal=10.0 padding_vertical=4.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_ADD.to_owned()}
                    label="Add a clip to the end of the base track"
                    @test_id={"video.add-clip"}
                    on_click={move || add.open_picker(None)}
                />
                <IconButton
                    glyph={ICON_SUBDIRECTORY_ARROW_RIGHT.to_owned()}
                    label="Attach a clip to the selected clip at the playhead"
                    disabled={none.clone()}
                    on_click={move || {
                        let selected = attach.selected.get_untracked();
                        attach.open_picker(selected);
                    }}
                />
                <IconButton
                    glyph={ICON_CONTENT_CUT.to_owned()}
                    label="Split the selected clip in two at the playhead"
                    disabled={none.clone()}
                    @test_id={"video.split"}
                    on_click={move || split.split_selected()}
                />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Delete the selected clip and everything attached to it"
                    disabled={none}
                    @test_id={"video.delete"}
                    on_click={move || remove.remove_selected()}
                />
                <Separator direction=Direction::Vertical length=TOOL_SEPARATOR_LENGTH />
                <IconButton
                    glyph={ICON_ZOOM_OUT.to_owned()}
                    label="Zoom the timeline out"
                    on_click={move || out.zoom_timeline(1.0 / ZOOM_STEP)}
                />
                <IconButton
                    glyph={ICON_ZOOM_IN.to_owned()}
                    label="Zoom the timeline in"
                    on_click={move || inward.zoom_timeline(ZOOM_STEP)}
                />
                <IconButton
                    glyph={ICON_FIT_SCREEN.to_owned()}
                    label="Fit the whole video in the timeline"
                    on_click={move || fit.request_fit()}
                />
            </List>
        </Frame>
    }
}

fn frame_rate_label(frame_rate: VideoFrameRate) -> String {
    if frame_rate.denominator == 1 {
        return format!("{} fps", frame_rate.numerator);
    }
    format!("{:.2} fps", frame_rate.frames_per_second())
}
