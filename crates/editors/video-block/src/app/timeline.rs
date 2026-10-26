use beui::Overflow;
use std::cell::Cell;
use std::rc::Rc;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{CursorIcon, NodeId, PointerPress, Pos2, Rect, ScrollGesture, Vec2};
use block_editor_beui::{ChildBlock, ChildMode, Drag};
use uuid::Uuid;

use crate::timeline::{
    self, ClipRow, LANE_GAP, LANE_HEIGHT, RULER_HEIGHT, TAIL_PADDING, TRIM_HANDLE_WIDTH,
    TimelineDropTarget,
};

use super::state::{ClipDrag, VideoState};

const THUMBNAIL_MARGIN: Space = Space::Xs2;
const CLIP_RADIUS: Radius = Radius::Sm;

#[component]
pub(crate) fn Timeline(state: VideoState) -> NodeId {
    view! {
        <PlacementBuilder>|placed: ReadSignal<Rect>| {
            create_effect(move || state.fit_timeline(placed.get().width().max(1.0)));

            let clips = state.clips;
            let rows = create_memo(move || {
                let _ = clips.get();
                state
                    .video()
                    .map(|video| {
                        timeline::lane_rows(&video)
                            .into_iter()
                            .map(|row| Lane {
                                id: row.timing.id,
                                lane: row.lane,
                                start: row.timing.start,
                                length: row.timing.length,
                                depth: row.timing.depth,
                            })
                            .collect::<Vec<Lane>>()
                    })
                    .unwrap_or_default()
            });
            let lanes = create_memo(move || {
                rows.get().iter().map(|row| row.lane + 1).max().unwrap_or(1) + 1
            });
            let ids = create_memo(move || rows.get().iter().map(|row| row.id).collect::<Vec<Uuid>>());

            let offset = create_signal(Vec2::ZERO);
            let camera = create_memo(move || {
                Some(CanvasView::new(placed.get().min - offset.get(), 1.0))
            });

            let pixels_per_frame = state.pixels_per_frame;
            let duration = state.duration;
            let content = create_memo(move || {
                let viewport = placed.get().size().max(Vec2::new(1.0, 1.0));
                Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(
                        (duration.get() as f32 * pixels_per_frame.get() + TAIL_PADDING).max(viewport.x),
                        (RULER_HEIGHT + lanes.get() as f32 * (LANE_HEIGHT + LANE_GAP)).max(viewport.y),
                    ),
                )
            });

            let on_scroll = move |gesture: ScrollGesture| {
                let content = content.get_untracked().size();
                let viewport = placed.get_untracked().size();
                let room = (content - viewport).max(Vec2::ZERO);
                let next = offset.get_untracked() - gesture.delta;
                offset.set(Vec2::new(
                    next.x.clamp(0.0, room.x),
                    next.y.clamp(0.0, room.y),
                ));
            };

            let pointer = create_signal(None::<Pos2>);
            let on_hover = move |press: PointerPress| {
                pointer.set(Some(press.local + offset.get_untracked()));
            };

            let target_drag = state.drag;
            let host_drag = state.editor().drag();
            let target = create_memo(move || {
                    let _ = rows.get();
                    let video = state.video()?;
                    let rows: Vec<ClipRow> = timeline::lane_rows(&video);
                    let content = content.get();
                    let scale = pixels_per_frame.get();
                    if let Some(drag) = target_drag.get() {
                        let at = pointer.get()?;
                        return timeline::drop_target_at(&video, &rows, content, scale, at, Some(&drag));
                    }
                    let dragged = host_drag.get().filter(|drag| drag.block_id != state.block_id())?;
                    let at = dragged.position - placed.get_untracked().min.to_vec2()
                        + offset.get_untracked();
                    timeline::drop_target_at(&video, &rows, content, scale, at, None)
                });

            let landing_drag = state.editor().drag();
            create_effect(move || {
                let dragged = landing_drag.get();
                untrack(|| land(state, target, dragged));
            });

            let on_drop = move |_: (Uuid, DragPoint)| {
                let Some(drag) = state.drag.get_untracked() else {
                    return;
                };
                let (Some(video), Some(target)) = (state.video(), target.get_untracked()) else {
                    return;
                };
                let mut operations = Vec::new();
                timeline::apply_clip_drop(&video, drag.clip, target, &mut operations);
                for operation in operations {
                    state.operate(operation);
                }
            };

            let on_over = move |over: Option<(Uuid, DragPoint)>| {
                if let Some((_, point)) = over {
                    pointer.set(Some(point.local + offset.get_untracked()));
                }
            };

            let on_press = move |press: PointerPress| {
                let x = press.local.x + offset.get_untracked().x;
                state.seek((x / pixels_per_frame.get_untracked()).max(0.0) as u64);
                state.select(None);
            };

            let ticks = create_memo(move || {
                let rate = state.frame_rate.get();
                let step = timeline::tick_seconds(rate, pixels_per_frame.get());
                let per_second = (rate.frames_per_second() * f64::from(pixels_per_frame.get())) as f32;
                let spacing = (step as f32 * per_second).max(1.0);
                let count = (content.get().width() / spacing).ceil() as u32;
                (0..=count)
                    .map(|tick| Tick {
                        index: tick,
                        x: tick as f32 * spacing,
                        seconds: (f64::from(tick) * step) as u64,
                    })
                    .collect::<Vec<Tick>>()
            });

            let lane_keys = create_memo(move || (0..lanes.get()).collect::<Vec<usize>>());
            let selection = create_selector(move || state.selected.get());
            let playhead = state.playhead;
            let playhead_x = create_memo(move || {
                playhead.get() as f32 * pixels_per_frame.get()
            });
            let playhead_height = create_memo(move || content.get().height());

            view! {
                <DropTarget on_over={on_over} on_drop={on_drop}>|_: DropHandle|
                    <Interactive
                        cursor=CursorIcon::Default
                        on_press={on_press}
                        on_scroll={on_scroll}
                        on_hover_move={on_hover}
                        @test_id={"video.timeline"}
                    >
                        <Canvas view={camera}>
                            <ForEach keys={lane_keys}>|lane: usize|
                                <LaneStrip lane content />
                            </ForEach>
                            <ForEach keys={ticks}>|tick: Tick|
                                <RulerTick tick />
                            </ForEach>
                            <ForEach keys={ids}>|id: Uuid| {
                                let selected = selection.memo(Some(id));
                                let row = create_memo(move || {
                                    rows.get().into_iter().find(|row| row.id == id).unwrap_or(Lane {
                                        id,
                                        lane: 0,
                                        start: 0,
                                        length: 0,
                                        depth: 0,
                                    })
                                });
                                view! {
                                    <Clip
                                        state
                                        id
                                        row
                                        selected
                                        scale={pixels_per_frame.read_only()}
                                    />
                                }
                            }</ForEach>
                            <DropMarker target={target} scale={pixels_per_frame.read_only()} />
                            <CanvasItem x={playhead_x} y=0.0 width=1.5 height={playhead_height}>
                                <Frame fill=ColorRole::Accent />
                            </CanvasItem>
                        </Canvas>
                    </Interactive>
                </DropTarget>
            }
        }</PlacementBuilder>
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Lane {
    id: Uuid,
    lane: usize,
    start: u64,
    length: u64,
    depth: usize,
}

#[derive(Clone, Copy, PartialEq)]
struct Tick {
    index: u32,
    x: f32,
    seconds: u64,
}

impl Eq for Tick {}

impl std::hash::Hash for Tick {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.index.hash(hasher);
    }
}

#[component]
fn LaneStrip(lane: usize, content: Memo<Rect>) -> CanvasItem {
    let color = match lane {
        0 => ColorRole::SurfaceRaised,
        _ => ColorRole::Surface,
    };
    let y = RULER_HEIGHT + lane as f32 * (LANE_HEIGHT + LANE_GAP);
    let width = create_memo(move || content.get().width());
    view! {
        <CanvasItem x=0.0 y={y} width={width} height=LANE_HEIGHT>
            <Frame fill=color radius=Radius::Xs />
        </CanvasItem>
    }
}

#[component]
fn RulerTick(tick: Tick) -> CanvasItem {
    let label = format!("{}:{:02}", tick.seconds / 60, tick.seconds % 60);
    view! {
        <CanvasItem x={tick.x + 3.0} y=2.0 width=60.0 height={RULER_HEIGHT - 4.0}>
            <Text style=TextStyle::CAPTION>{label}</Text>
        </CanvasItem>
    }
}

#[component]
fn Clip(
    state: VideoState,
    id: Uuid,
    row: Memo<Lane>,
    selected: Memo<bool>,
    scale: ReadSignal<f32>,
) -> CanvasItem {
    let x = create_memo(move || row.get().start as f32 * scale.get());
    let width = create_memo(move || (row.get().length as f32 * scale.get()).max(3.0));
    let y = create_memo(move || RULER_HEIGHT + row.get().lane as f32 * (LANE_HEIGHT + LANE_GAP));
    let clips = state.clips;
    let block = create_memo(move || {
        let clip = clips.get().into_iter().find(|clip| clip.id == id)?;
        state.target(clip.block_id)
    });
    let name = create_memo(move || {
        clips
            .get()
            .into_iter()
            .find(|clip| clip.id == id)
            .map_or_else(String::new, |clip| state.name_of(clip.block_id))
    });
    let attached = create_memo(move || {
        clips
            .get()
            .iter()
            .find(|clip| clip.id == id)
            .is_some_and(|clip| clip.attachment.is_some())
    });
    let fill = create_memo(move || match attached.get() {
        true => ColorRole::Hover,
        false => ColorRole::Pressed,
    });
    let editor = state.editor();
    let element_test_id = format!("video.clip.{}", id);
    let grabbed_at = Rc::new(Cell::new(None::<Pos2>));
    let grabbing = Rc::clone(&grabbed_at);

    view! {
        <CanvasItem x={x} y={y} width={width} height=LANE_HEIGHT>
            <Draggable
                payload={id}
                cursor=CursorIcon::Grab
                @test_id={element_test_id}
                on_click={move || state.select(Some(id))}
                on_drag_change={move |dragging: bool| {
                    if !dragging {
                        state.begin_drag(None);
                        return;
                    }
                    let scale = scale.get_untracked().max(f32::EPSILON);
                    let pressed = grabbed_at.get().map_or(0.0, |pos| pos.x);
                    let offset = pressed / scale;
                    state.begin_drag(Some(ClipDrag {
                        clip: id,
                        grab: offset.max(0.0) as u64,
                    }));
                }}
            >|_: DragHandle|
                <Interactive on_press={move |press: PointerPress| grabbing.set(Some(press.local))}>
                    <Frame
                        fill={fill}
                        radius=CLIP_RADIUS
                        border={create_memo(move || selected.get().then_some(Border::FOCUS))}
                        padding=THUMBNAIL_MARGIN
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                            <Frame
                                width={Size::Custom((LANE_HEIGHT - THUMBNAIL_MARGIN.px() * 2.0) * 1.4)}
                                height={Size::Custom(LANE_HEIGHT - THUMBNAIL_MARGIN.px() * 2.0)}
                            >
                                <ChildBlock
                                    editor={editor}
                                    block={block}
                                    mode=ChildMode::Preview
                                    on_state={move |_| {}}
                                />
                            </Frame>
                            <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>
                                {name}
                            </Text>
                            <TrimHandle state={state} id={id} scale={scale} />
                        </List>
                    </Frame>
                </Interactive>
            </Draggable>
        </CanvasItem>
    }
}

#[component]
fn DropMarker(
    target: Memo<Option<TimelineDropTarget>>,
    scale: ReadSignal<f32>,
) -> DynamicSegment<CanvasItem> {
    let preview = create_memo(move || {
        let scale = scale.get();
        match target.get()? {
            TimelineDropTarget::Attach {
                start,
                lane,
                length,
                ..
            }
            | TimelineDropTarget::Offset {
                start,
                lane,
                length,
            } => {
                let row = timeline::lane_rect(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO), lane);
                let left = start as f32 * scale;
                Some(Rect::from_min_max(
                    Pos2::new(left, row.top()),
                    Pos2::new(left + (length as f32 * scale).max(3.0), row.bottom()),
                ))
            }
            TimelineDropTarget::Base { x, .. } => {
                let row = timeline::lane_rect(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO), 0);
                Some(Rect::from_min_max(
                    Pos2::new(x - 1.5, row.top()),
                    Pos2::new(x + 1.5, row.bottom()),
                ))
            }
        }
    });
    let shown = create_memo(move || preview.get().is_some());
    let rect = create_memo(move || preview.get().unwrap_or(Rect::ZERO));
    let x = create_memo(move || rect.get().left());
    let y = create_memo(move || rect.get().top());
    let width = create_memo(move || rect.get().width());
    let height = create_memo(move || rect.get().height());
    view! {
        <Show condition={shown}>
            <CanvasItem x={x} y={y} width={width} height={height}>
                <Frame fill=ColorRole::AccentSoft radius=CLIP_RADIUS border={Border::FOCUS} />
            </CanvasItem>
        </Show>
    }
}

#[component]
fn TrimHandle(state: VideoState, id: Uuid, scale: ReadSignal<f32>) -> NodeId {
    let clips = state.clips;
    let length_of = move || clips.get_untracked().into_iter().find(|clip| clip.id == id);
    let grabbed = Rc::new(Cell::new(None::<(f32, u64)>));
    let held = Rc::clone(&grabbed);
    let pressed = move |press: PointerPress| {
        held.set(length_of().map(|clip| (press.pos.x, clip.length)));
    };
    view! {
        <Interactive
            cursor=CursorIcon::ResizeHorizontal
            capture_presses=true
            @test_id={format!("video.trim.{}", id)}
            on_press={pressed}
            on_drag={move |press: PointerPress| {
                let scale = scale.get_untracked().max(f32::EPSILON);
                let (Some((from, start)), Some(mut clip)) = (grabbed.get(), length_of()) else {
                    return;
                };
                let reach = start as f32 + (press.pos.x - from) / scale;
                let length = (reach.max(1.0) as u64).max(1);
                if length != clip.length {
                    clip.length = length;
                    state.update_clip(clip);
                }
            }}
        >
            <Frame width={Size::Custom(TRIM_HANDLE_WIDTH)} height={Size::Custom(LANE_HEIGHT)} />
        </Interactive>
    }
}

fn land(landing: VideoState, target: Memo<Option<TimelineDropTarget>>, dragged: Option<Drag>) {
    let Some(dragged) = dragged.filter(|drag| drag.block_id != landing.block_id()) else {
        return;
    };
    let target = target.get_untracked();
    landing.editor().accept_drag(target.is_some());
    if !dragged.dropped {
        return;
    }
    match target {
        Some(TimelineDropTarget::Attach { parent, start, .. }) => {
            landing.adopt(dragged.block_id);
            landing.insert_clip(dragged.block_id, Some(parent), start, Some(0));
        }
        Some(TimelineDropTarget::Base { index, .. }) => {
            landing.adopt(dragged.block_id);
            landing.insert_clip(dragged.block_id, None, 0, Some(index));
        }
        Some(TimelineDropTarget::Offset { .. }) | None => {}
    }
}
