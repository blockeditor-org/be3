use video_block_types::VideoContent;

use block_editor_beui::Loaded;
use block_editor_beui::beui::icons::{
    ICON_CORNER_DOWN_RIGHT, ICON_PAUSE, ICON_PLAY, ICON_PLUS, ICON_SCAN, ICON_SCISSORS,
    ICON_SKIP_BACK, ICON_SKIP_FORWARD, ICON_TRASH, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{Creation, Editor, Toolbar};
use uuid::Uuid;
use video_block_types::{VideoFrameRate, VideoOperation};

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

#[component]
impl block_editor_beui::BeuiApp for VideoApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <VideoEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <VideoPreview editor={editor} />
            </Loaded>
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
    let state = VideoState::new(editor);

    let chrome = editor.chrome_shown();

    view! {
        <List spacing=0.0>
            <FrameRateBar state={state} shown={chrome} />
            <List
                @sizing=ItemSize::Percent(100.0 - TIMELINE_SHARE)
                direction=Direction::Horizontal
                spacing=0.0
            >
                <EffectsColumn state={state} />
                <Separator direction=Direction::Vertical />
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <Player @sizing=ItemSize::Percent(100.0) state={state} />
                    <Separator />
                    <PlaybackBar state={state} />
                </List>
            </List>
            <Separator />
            <TimelineTools state={state} />
            <Separator />
            <Timeline @sizing=ItemSize::Percent(TIMELINE_SHARE) state={state} />
        </List>
    }
}

#[component]
fn VideoPreview(editor: Editor) -> NodeId {
    let state = VideoState::new(editor);
    view! {
        <Player state={state} />
    }
}

#[component]
fn EffectsColumn(state: VideoState) -> NodeId {
    view! {
        <Frame width=EFFECTS_WIDTH padding=10.0>
            <Scroll>
                <EffectsPanel state={state} />
            </Scroll>
        </Frame>
    }
}

#[component]
fn FrameRateBar(state: VideoState, shown: Prop<bool>) -> DynamicSegment<ListChild> {
    let rate = state.frame_rate;
    let chosen = create_memo(move || {
        FRAME_RATES
            .into_iter()
            .position(|option| option == rate.get())
    });
    let options = (0..FRAME_RATES.len()).collect::<Vec<usize>>();
    view! {
        <Toolbar shown={shown}>
            <Select
                options={view! {
                    <ForEach keys={options}>|index: usize|
                        <ChoiceOption label={frame_rate_label(FRAME_RATES[index])} />
                    </ForEach>
                }}
                selected={Binding::new(chosen, move |index: Option<usize>| {
                    let Some(frame_rate) = index.and_then(|index| FRAME_RATES.get(index).copied())
                    else {
                        return;
                    };
                    state.operate(VideoOperation::SetFrameRate { frame_rate });
                })}
                label="Frames per second"
                @test_id={"video.frame-rate"}
            />
        </Toolbar>
    }
}

#[component]
fn PlaybackBar(state: VideoState) -> NodeId {
    let playing = state.playing;
    let empty = create_memo(move || state.duration.get() == 0);
    let glyph = create_memo(move || match playing.get() {
        true => ICON_PAUSE,
        false => ICON_PLAY,
    });
    let playhead = state.playhead;
    let duration = state.duration;
    let rate = state.frame_rate;
    let position = create_memo(move || {
        format!(
            "{} / {}",
            crate::timeline::timecode(rate.get(), playhead.get()),
            crate::timeline::timecode(rate.get(), duration.get())
        )
    });
    view! {
        <Frame padding={Sides::symmetric(10.0, 4.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph=ICON_SKIP_BACK
                    label="Go to the start"
                    on_click={move || state.seek(0)}
                />
                <IconButton
                    glyph={glyph}
                    label="Play"
                    disabled={empty}
                    @test_id={"video.play"}
                    on_click={move || state.toggle_playback()}
                />
                <IconButton
                    glyph=ICON_SKIP_FORWARD
                    label="Go to the end"
                    on_click={move || state.seek(state.duration.get_untracked())}
                />
                <Text style=TextStyle::CAPTION>{position}</Text>
            </List>
        </Frame>
    }
}

#[component]
fn TimelineTools(state: VideoState) -> NodeId {
    let none = create_memo(move || state.selected.get().is_none());
    view! {
        <Frame padding={Sides::symmetric(10.0, 4.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph=ICON_PLUS
                    label="Add a clip to the end of the base track"
                    @test_id={"video.add-clip"}
                    on_click={move || state.open_picker(None)}
                />
                <IconButton
                    glyph=ICON_CORNER_DOWN_RIGHT
                    label="Attach a clip to the selected clip at the playhead"
                    disabled={none}
                    on_click={move || {
                        let selected = state.selected.get_untracked();
                        state.open_picker(selected);
                    }}
                />
                <IconButton
                    glyph=ICON_SCISSORS
                    label="Split the selected clip in two at the playhead"
                    disabled={none}
                    @test_id={"video.split"}
                    on_click={move || state.split_selected()}
                />
                <IconButton
                    glyph=ICON_TRASH
                    label="Delete the selected clip and everything attached to it"
                    disabled={none}
                    @test_id={"video.delete"}
                    on_click={move || state.remove_selected()}
                />
                <Separator direction=Direction::Vertical length=TOOL_SEPARATOR_LENGTH />
                <IconButton
                    glyph=ICON_ZOOM_OUT
                    label="Zoom the timeline out"
                    on_click={move || state.zoom_timeline(1.0 / ZOOM_STEP)}
                />
                <IconButton
                    glyph=ICON_ZOOM_IN
                    label="Zoom the timeline in"
                    on_click={move || state.zoom_timeline(ZOOM_STEP)}
                />
                <IconButton
                    glyph=ICON_SCAN
                    label="Fit the whole video in the timeline"
                    on_click={move || state.request_fit()}
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
