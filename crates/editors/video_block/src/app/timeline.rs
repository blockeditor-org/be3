use std::cell::Cell;
use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    Align, Canvas, CanvasItem, CanvasView, ClickCatcher, Direction, ForEach, Frame, ItemSize, List,
    Memo, ReadSignal, clone, component, component_rect, create_effect, create_memo,
    create_selector, create_signal, untrack, view,
};
use block_editor_plugin::beui::styled::{Body, Caption, use_theme};
use block_editor_plugin::beui::unstyled::{
    DragHandle, DragPoint, Draggable, DropHandle, DropTarget,
};
use block_editor_plugin::beui::{
    CursorIcon, NodeId, PointerPress, Pos2, Rect, ScrollGesture, Vec2,
};
use block_editor_plugin::{ChildBlock, ChildMode, Drag};
use uuid::Uuid;

use crate::timeline::{
    self, ClipRow, LANE_GAP, LANE_HEIGHT, RULER_HEIGHT, TAIL_PADDING, TRIM_HANDLE_WIDTH,
    TimelineDropTarget,
};

use super::state::{ClipDrag, VideoState};

const THUMBNAIL_MARGIN: f32 = 3.0;
const CLIP_RADIUS: u8 = 4;

#[component]
pub(crate) fn Timeline(state: Rc<VideoState>) -> NodeId {
    let placed = component_rect();
    let fitting = Rc::clone(&state);
    let fitted = placed.clone();
    create_effect(move || fitting.fit_timeline(fitted.get().width().max(1.0)));

    let rowed = Rc::clone(&state);
    let clips = state.clips.clone();
    let rows = create_memo(clone!(rowed clips -> move || {
        let _ = clips.get();
        rowed
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
    }));
    let lanes = create_memo(clone!(rows -> move || {
        rows.get().iter().map(|row| row.lane + 1).max().unwrap_or(1) + 1
    }));

    let (offset, set_offset) = create_signal(Vec2::ZERO);
    let camera = create_memo(clone!(offset placed -> move || {
        Some(CanvasView::new(placed.get().min - offset.get(), 1.0))
    }));

    let pixels_per_frame = state.pixels_per_frame.clone();
    let duration = state.duration.clone();
    let content = create_memo(clone!(duration pixels_per_frame lanes placed -> move || {
        let viewport = placed.get().size().max(Vec2::new(1.0, 1.0));
        Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(
                (duration.get() as f32 * pixels_per_frame.get() + TAIL_PADDING).max(viewport.x),
                (RULER_HEIGHT + lanes.get() as f32 * (LANE_HEIGHT + LANE_GAP)).max(viewport.y),
            ),
        )
    }));

    let scrolled = content.clone();
    let scroll_placed = placed.clone();
    let scroll_offset = offset.clone();
    let on_scroll = move |gesture: ScrollGesture| {
        let content = scrolled.get_untracked().size();
        let viewport = scroll_placed.get_untracked().size();
        let room = (content - viewport).max(Vec2::ZERO);
        let next = scroll_offset.get_untracked() - gesture.delta;
        set_offset.set(Vec2::new(
            next.x.clamp(0.0, room.x),
            next.y.clamp(0.0, room.y),
        ));
    };

    let (pointer, set_pointer) = create_signal(None::<Pos2>);
    let hover_offset = offset.clone();
    let hover_placed = placed.clone();
    let hover_pointer = set_pointer.clone();
    let on_hover = move |press: PointerPress| {
        hover_pointer.set(Some(
            press.pos - hover_placed.get_untracked().min.to_vec2() + hover_offset.get_untracked(),
        ));
    };

    let targeted = Rc::clone(&state);
    let target_rows = rows.clone();
    let target_scale = pixels_per_frame.clone();
    let target_pointer = pointer.clone();
    let target_drag = state.drag.clone();
    let target_offset = offset.clone();
    let target_placed = placed.clone();
    let host_drag = state.editor().drag();
    let content_rect = content.clone();
    let target = create_memo(clone!(
        targeted target_rows target_scale target_pointer target_drag host_drag content_rect
        target_placed target_offset
        -> move || {
            let _ = target_rows.get();
            let video = targeted.video()?;
            let rows: Vec<ClipRow> = timeline::lane_rows(&video);
            let content = content_rect.get();
            let scale = target_scale.get();
            if let Some(drag) = target_drag.get() {
                let at = target_pointer.get()?;
                return timeline::drop_target_at(&video, &rows, content, scale, at, Some(&drag));
            }
            let dragged = host_drag.get().filter(|drag| drag.block_id != targeted.block_id())?;
            let at = dragged.position - target_placed.get_untracked().min.to_vec2()
                + target_offset.get_untracked();
            timeline::drop_target_at(&video, &rows, content, scale, at, None)
        }
    ));

    let landing = Rc::clone(&state);
    let landing_target = target.clone();
    let landing_drag = state.editor().drag();
    create_effect(move || {
        let dragged = landing_drag.get();
        untrack(|| land(&landing, &landing_target, dragged));
    });

    let dropped = Rc::clone(&state);
    let dropped_target = target.clone();
    let on_drop = move |_: (Uuid, DragPoint)| {
        let Some(drag) = dropped.drag.get_untracked() else {
            return;
        };
        let (Some(video), Some(target)) = (dropped.video(), dropped_target.get_untracked()) else {
            return;
        };
        let mut operations = Vec::new();
        timeline::apply_clip_drop(&video, drag.clip, target, &mut operations);
        for operation in operations {
            dropped.operate(operation);
        }
    };

    let drag_offset = offset.clone();
    let drag_placed = placed.clone();
    let drag_pointer = set_pointer.clone();
    let on_over = move |over: Option<(Uuid, DragPoint)>| {
        if let Some((_, point)) = over {
            drag_pointer.set(Some(
                point.pos - drag_placed.get_untracked().min.to_vec2() + drag_offset.get_untracked(),
            ));
        }
    };

    let seeking = Rc::clone(&state);
    let seek_offset = offset.clone();
    let seek_placed = placed.clone();
    let seek_scale = pixels_per_frame.clone();
    let on_press = move |press: PointerPress| {
        let x = press.pos.x - seek_placed.get_untracked().min.x + seek_offset.get_untracked().x;
        seeking.seek((x / seek_scale.get_untracked()).max(0.0) as u64);
        seeking.select(None);
    };

    let ticks = create_memo(clone!(state content pixels_per_frame -> move || {
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
    }));

    let lane_keys = create_memo(clone!(lanes -> move || (0..lanes.get()).collect::<Vec<usize>>()));
    let selection = create_selector(clone!(state -> move || state.selected.get()));
    let rowed_state = Rc::clone(&state);
    let row_scale = pixels_per_frame.clone();
    let playhead = state.playhead.clone();
    let playhead_x = create_memo(clone!(playhead pixels_per_frame -> move || {
        playhead.get() as f32 * pixels_per_frame.get()
    }));
    let playhead_height = create_memo(clone!(content -> move || content.get().height()));
    let theme = use_theme();

    view! {
        <DropTarget on_over={on_over} on_drop={on_drop}>
            {move |_: DropHandle| view! {
                <ClickCatcher
                    cursor=CursorIcon::Default
                    on_press={on_press}
                    on_scroll={on_scroll}
                    on_hover_move={on_hover}
                    @test_id={"video.timeline"}
                >
                    <Canvas view={camera}>
                        <ForEach keys={lane_keys}>
                            {move |lane: usize| {
                                let content = content.clone();
                                view! {
                                    <LaneStrip lane content />
                                }
                            }}
                        </ForEach>
                        <ForEach keys={ticks}>
                            {move |tick: Tick| view! {
                                <RulerTick tick />
                            }}
                        </ForEach>
                        <ForEach keys={rows}>
                            {move |row: Lane| {
                                let state = Rc::clone(&rowed_state);
                                let selected = selection.memo(Some(row.id));
                                let scale = row_scale.clone();
                                view! {
                                    <Clip state row selected scale />
                                }
                            }}
                        </ForEach>
                        <DropMarker target={target} scale={pixels_per_frame.clone()} />
                        <CanvasItem x={playhead_x} y=0.0 width=1.5 height={playhead_height}>
                            <Frame color={theme.accent.clone()} />
                        </CanvasItem>
                    </Canvas>
                </ClickCatcher>
            }}
        </DropTarget>
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
    let theme = use_theme();
    let color = match lane {
        0 => theme.surface_raised.clone(),
        _ => theme.surface.clone(),
    };
    let y = RULER_HEIGHT + lane as f32 * (LANE_HEIGHT + LANE_GAP);
    let width = create_memo(clone!(content -> move || content.get().width()));
    view! {
        <CanvasItem x=0.0 y={y} width={width} height=LANE_HEIGHT>
            <Frame color={color} radius=3 />
        </CanvasItem>
    }
}

#[component]
fn RulerTick(tick: Tick) -> CanvasItem {
    let theme = use_theme();
    let label = format!("{}:{:02}", tick.seconds / 60, tick.seconds % 60);
    view! {
        <CanvasItem x={tick.x + 3.0} y=2.0 width=60.0 height={RULER_HEIGHT - 4.0}>
            <Caption content={label} color={theme.text_muted.clone()} />
        </CanvasItem>
    }
}

#[component]
fn Clip(
    state: Rc<VideoState>,
    row: Lane,
    selected: Memo<bool>,
    scale: ReadSignal<f32>,
) -> CanvasItem {
    let x = create_memo(clone!(scale -> move || row.start as f32 * scale.get()));
    let width = create_memo(clone!(scale -> move || {
        (row.length as f32 * scale.get()).max(3.0)
    }));
    let y = RULER_HEIGHT + row.lane as f32 * (LANE_HEIGHT + LANE_GAP);
    let held = Rc::clone(&state);
    let clips = state.clips.clone();
    let block = create_memo(clone!(held clips -> move || {
        let clip = clips.get().into_iter().find(|clip| clip.id == row.id)?;
        held.target(clip.block_id)
    }));
    let named = Rc::clone(&state);
    let name = create_memo(clone!(clips named -> move || {
        clips
            .get()
            .into_iter()
            .find(|clip| clip.id == row.id)
            .map_or_else(String::new, |clip| named.name_of(clip.block_id))
    }));
    let attached = create_memo(clone!(clips -> move || {
        clips
            .get()
            .iter()
            .find(|clip| clip.id == row.id)
            .is_some_and(|clip| clip.attachment.is_some())
    }));
    let theme = use_theme();
    let fill = create_memo(clone!(theme attached -> move || match attached.get() {
        true => theme.hover.get(),
        false => theme.pressed.get(),
    }));
    let chosen = Rc::clone(&state);
    let trimming = Rc::clone(&state);
    let trim_scale = scale.clone();
    let clip_placed = component_rect();
    let clip_left = create_memo(clone!(clip_placed -> move || clip_placed.get().left()));
    let handle_left = clip_left.clone();
    let trimming_edge = Rc::clone(&state);
    let trim_edge_scale = scale.clone();
    let editor = state.editor().clone();
    let test_id = format!("video.clip.{}", row.id);
    let grabbed_at = Rc::new(Cell::new(None::<Pos2>));
    let grabbing = Rc::clone(&grabbed_at);

    view! {
        <CanvasItem x={x} y={y} width={width} height=LANE_HEIGHT>
            <Draggable
                payload={row.id}
                cursor=CursorIcon::Grab
                @test_id={test_id}
                on_click={move || chosen.select(Some(row.id))}
                on_drag_change={move |dragging: bool| {
                    if !dragging {
                        trimming.begin_drag(None);
                        return;
                    }
                    let scale = trim_scale.get_untracked().max(f32::EPSILON);
                    let pressed = grabbed_at.get().map_or(0.0, |pos| pos.x);
                    let offset = (pressed - clip_left.get_untracked()) / scale;
                    trimming.begin_drag(Some(ClipDrag {
                        clip: row.id,
                        grab: offset.max(0.0) as u64,
                    }));
                }}
            >
                {move |_: DragHandle| view! {
                    <ClickCatcher
                        on_press={move |press: PointerPress| grabbing.set(Some(press.pos))}
                    >
                        <Frame
                            color={fill}
                            radius=CLIP_RADIUS
                            outline={theme.accent.clone()}
                            outline_width=2.0
                            outline_visible={selected}
                            padding_horizontal=THUMBNAIL_MARGIN
                            padding_vertical=THUMBNAIL_MARGIN
                        >
                            <List direction=Direction::Horizontal align=Align::Center spacing=5.0>
                                <Frame
                                    width={(LANE_HEIGHT - THUMBNAIL_MARGIN * 2.0) * 1.4}
                                    height={LANE_HEIGHT - THUMBNAIL_MARGIN * 2.0}
                                >
                                    <ChildBlock
                                        editor={editor}
                                        block={block}
                                        mode=ChildMode::Preview
                                        on_state={move |_| {}}
                                    />
                                </Frame>
                                <Body @sizing=ItemSize::Percent(100.0) content={name} />
                                <TrimHandle
                                    state={trimming_edge}
                                    row={row}
                                    scale={trim_edge_scale}
                                    left={handle_left}
                                />
                            </List>
                        </Frame>
                    </ClickCatcher>
                }}
            </Draggable>
        </CanvasItem>
    }
}

#[component]
fn DropMarker(target: Memo<Option<TimelineDropTarget>>, scale: ReadSignal<f32>) -> CanvasItem {
    let preview = create_memo(clone!(target scale -> move || {
        let scale = scale.get();
        match target.get()? {
            TimelineDropTarget::Attach {
                start, lane, length, ..
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
    }));
    let shown = create_memo(clone!(preview -> move || preview.get().is_some()));
    let rect = create_memo(clone!(preview -> move || preview.get().unwrap_or(Rect::ZERO)));
    let x = create_memo(clone!(rect -> move || rect.get().left()));
    let y = create_memo(clone!(rect -> move || rect.get().top()));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    let theme = use_theme();
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame
                visible={shown.clone()}
                color={theme.accent_soft.clone()}
                radius=CLIP_RADIUS
                outline={theme.accent.clone()}
                outline_width=2.0
                outline_visible={shown}
            />
        </CanvasItem>
    }
}

#[component]
fn TrimHandle(state: Rc<VideoState>, row: Lane, scale: ReadSignal<f32>, left: Memo<f32>) -> NodeId {
    let clips = state.clips.clone();
    view! {
        <ClickCatcher
            cursor=CursorIcon::ResizeHorizontal
            capture_presses=true
            @test_id={format!("video.trim.{}", row.id)}
            on_drag={move |press: PointerPress| {
                let scale = scale.get_untracked().max(f32::EPSILON);
                let Some(mut clip) = clips
                    .get_untracked()
                    .into_iter()
                    .find(|clip| clip.id == row.id)
                else {
                    return;
                };
                let reach = (press.pos.x - left.get_untracked()) / scale;
                let length = (reach.max(1.0) as u64).max(1);
                if length != clip.length {
                    clip.length = length;
                    state.update_clip(clip);
                }
            }}
        >
            <Frame width=TRIM_HANDLE_WIDTH height=LANE_HEIGHT />
        </ClickCatcher>
    }
}

fn land(landing: &VideoState, target: &Memo<Option<TimelineDropTarget>>, dragged: Option<Drag>) {
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
