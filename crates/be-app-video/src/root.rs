use be_type_video::VideoContent;

use be_sdk_beui::Loaded;
use be_sdk_beui::beui::dock::styled::{
    DockBar, DockBarGrip, DockBarSurface, DockPane, DockSplit, DockTab, Docking, DockingLayout,
    SplitSize, use_dock_bar,
};
use be_sdk_beui::beui::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::icons::{
    ICON_CORNER_DOWN_RIGHT, ICON_PAUSE, ICON_PLAY, ICON_PLUS, ICON_SCAN, ICON_SCISSORS,
    ICON_SKIP_BACK, ICON_SKIP_FORWARD, ICON_TRASH, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{Creation, Editor};
use be_type_video::{VideoFrameRate, VideoOperation};
use uuid::Uuid;

pub(crate) mod effects;
pub(crate) mod player;
pub(crate) mod state;
pub(crate) mod timeline;

use beui::Direction;
use effects::EffectsPanel;
use player::Player;
use state::{VideoState, ZOOM_STEP};
use timeline::Timeline;

const DEFAULT_EDITOR_SIZE: Vec2 = Vec2::new(1000.0, 640.0);
const EFFECTS_SHARE: f32 = 0.27;
const TIMELINE_SHARE: f32 = 0.42;
const TOOL_SEPARATOR_LENGTH: Size = Size::Sm;
const EFFECTS_PADDING: Space = Space::Md;
const PLAYER: &str = "player";
const EFFECTS: &str = "effects";
const TIMELINE: &str = "timeline";
const FRAME_RATE_BAR: &str = "frame-rate";
const PLAYBACK_BAR: &str = "playback";
const TIMELINE_BAR: &str = "timeline-tools";

const FRAME_RATES: [VideoFrameRate; 7] = [
    VideoFrameRate::new(24, 1),
    VideoFrameRate::new(24_000, 1001),
    VideoFrameRate::new(25, 1),
    VideoFrameRate::new(30, 1),
    VideoFrameRate::new(30_000, 1001),
    VideoFrameRate::new(50, 1),
    VideoFrameRate::new(60, 1),
];

pub struct VideoRoot;

#[component]
impl be_sdk_beui::BeuiRoot for VideoRoot {
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
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(PLAYER)}>
            <DockSplit
                id="video"
                direction=Direction::Vertical
                size={SplitSize::Fraction(1.0 - TIMELINE_SHARE)}
            >
                <DockSplit id="upper" size={SplitSize::Fraction(EFFECTS_SHARE)}>
                    <DockPane id="effects" locked=true>
                        <DockTab id=EFFECTS title="Effects">
                            <EffectsColumn state={state} />
                        </DockTab>
                    </DockPane>
                    <DockPane id="player" locked=true bare=true>
                        <DockTab id=PLAYER title="Player">
                            <Player state={state} />
                        </DockTab>
                    </DockPane>
                </DockSplit>
                <DockPane id="timeline" locked=true bare=true>
                    <DockTab id=TIMELINE title="Timeline">
                        <Timeline state={state} />
                    </DockTab>
                </DockPane>
            </DockSplit>
            <Show condition={chrome}>
                <DockBar
                    id=FRAME_RATE_BAR
                    parent=PLAYER
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <FrameRateBar state={state} />
                </DockBar>
            </Show>
            <DockBar
                id=PLAYBACK_BAR
                parent=PLAYER
                placement={DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Start)}
            >
                <PlaybackBar state={state} />
            </DockBar>
            <DockBar
                id=TIMELINE_BAR
                parent=TIMELINE
                placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
            >
                <TimelineTools state={state} />
            </DockBar>
        </Docking>
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
        <Frame fill=ColorRole::Surface>
            <Scroll @test_id={"video.effects"}>
                <Frame padding=EFFECTS_PADDING>
                    <EffectsPanel state={state} />
                </Frame>
            </Scroll>
        </Frame>
    }
}

#[component]
fn FrameRateBar(state: VideoState) -> NodeId {
    let rate = state.frame_rate;
    let chosen = create_memo(move || {
        FRAME_RATES
            .into_iter()
            .position(|option| option == rate.get())
    });
    let options = (0..FRAME_RATES.len()).collect::<Vec<usize>>();
    view! {
        <DockBarSurface spacing=Space::Sm @test_id={"video.frame-rate-bar"}>
            <Frame @test_id={"video.frame-rate-bar.grip"}>
                <DockBarGrip />
            </Frame>
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
        </DockBarSurface>
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
        <DockBarSurface spacing=Space::Sm @test_id={"video.playback"}>
            <Frame @test_id={"video.playback.grip"}>
                <DockBarGrip />
            </Frame>
            <Button
                icon_only=true
                glyph=ICON_SKIP_BACK
                label="Go to the start"
                on_click={move || state.seek(0)}
            />
            <Button
                icon_only=true
                glyph={glyph}
                label="Play"
                disabled={empty}
                @test_id={"video.play"}
                on_click={move || state.toggle_playback()}
            />
            <Button
                icon_only=true
                glyph=ICON_SKIP_FORWARD
                label="Go to the end"
                on_click={move || state.seek(state.duration.get_untracked())}
            />
            <Text style=TextStyle::CAPTION>{position}</Text>
        </DockBarSurface>
    }
}

#[component]
fn TimelineTools(state: VideoState) -> NodeId {
    let none = create_memo(move || state.selected.get().is_none());
    let bar = use_dock_bar();
    let across = create_memo(move || match bar.map(|bar| bar.direction()) {
        Some(Direction::Vertical) => Direction::Horizontal,
        _ => Direction::Vertical,
    });
    view! {
        <DockBarSurface spacing=Space::Sm @test_id={"video.timeline-tools"}>
            <Frame @test_id={"video.timeline-tools.grip"}>
                <DockBarGrip />
            </Frame>
            <Button
                icon_only=true
                glyph=ICON_PLUS
                label="Add a clip to the end of the base track"
                @test_id={"video.add-clip"}
                on_click={move || state.open_picker(None)}
            />
            <Button
                icon_only=true
                glyph=ICON_CORNER_DOWN_RIGHT
                label="Attach a clip to the selected clip at the playhead"
                disabled={none}
                on_click={move || {
                    let selected = state.selected.get_untracked();
                    state.open_picker(selected);
                }}
            />
            <Button
                icon_only=true
                glyph=ICON_SCISSORS
                label="Split the selected clip in two at the playhead"
                disabled={none}
                @test_id={"video.split"}
                on_click={move || state.split_selected()}
            />
            <Button
                icon_only=true
                glyph=ICON_TRASH
                label="Delete the selected clip and everything attached to it"
                disabled={none}
                @test_id={"video.delete"}
                on_click={move || state.remove_selected()}
            />
            <Separator direction={across} length=TOOL_SEPARATOR_LENGTH />
            <Button
                icon_only=true
                glyph=ICON_ZOOM_OUT
                label="Zoom the timeline out"
                on_click={move || state.zoom_timeline(1.0 / ZOOM_STEP)}
            />
            <Button
                icon_only=true
                glyph=ICON_ZOOM_IN
                label="Zoom the timeline in"
                on_click={move || state.zoom_timeline(ZOOM_STEP)}
            />
            <Button
                icon_only=true
                glyph=ICON_SCAN
                label="Fit the whole video in the timeline"
                on_click={move || state.request_fit()}
            />
        </DockBarSurface>
    }
}

fn frame_rate_label(frame_rate: VideoFrameRate) -> String {
    if frame_rate.denominator == 1 {
        return format!("{} fps", frame_rate.numerator);
    }
    format!("{:.2} fps", frame_rate.frames_per_second())
}
