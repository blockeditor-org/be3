use be_sdk_beui::beui::CanvasView;
use beui::Overflow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{
    CursorIcon, NodeId, PointerId, PointerPress, Pos2, Rect, ScrollGesture, Vec2, accesskit,
    drag,
};
use be_sdk_beui::{ChildBlock, ChildMode, Drag};
use uuid::Uuid;

use crate::timeline::{
    self, ClipRow, LANE_GAP, LANE_HEIGHT, RULER_HEIGHT, TAIL_PADDING, TRIM_HANDLE_WIDTH,
    TimelineDropTarget,
};

use super::state::{ClipDrag, VideoState};
use beui::{Align, Direction};

const THUMBNAIL_MARGIN: Space = Space::Xs2;
const CLIP_RADIUS: Radius = Radius::Sm;

/// How far apart, in points, a keyboard carry's stops along a lane are at the least.
const KEYBOARD_STOP_SPACING: f32 = 8.0;

/// The stops a clip carried from the keyboard steps between, in the timeline's content:
/// a grid of frames on each lane, at least `KEYBOARD_STOP_SPACING` apart, where the clip would
/// land, each named by its lane and the time it starts at.
fn keyboard_stops(
    video: &be_type_video::Video,
    rows: &[ClipRow],
    content: Rect,
    pixels_per_frame: f32,
    (lanes, frame_rate): (usize, be_type_video::VideoFrameRate),
    dragged: &ClipDrag,
) -> Vec<drag::Stop> {
    let scale = pixels_per_frame.max(f32::EPSILON);
    let stride = (KEYBOARD_STOP_SPACING / scale).ceil().max(1.0) as u64;
    let frames = (content.width() / scale) as u64;
    let width = stride as f32 * scale;
    (0..lanes)
        .flat_map(|lane| {
            let y = RULER_HEIGHT + lane as f32 * (LANE_HEIGHT + LANE_GAP) + LANE_HEIGHT / 2.0;
            (0..=frames).step_by(stride as usize).filter_map(move |frame| {
                let at = Pos2::new(content.left() + frame as f32 * scale, y);
                timeline::drop_target_at(video, rows, content, scale, at, Some(dragged))?;
                let start = frame.saturating_sub(dragged.grab);
                Some(drag::Stop::new(
                    Rect::from_center_size(at, Vec2::new(width, LANE_HEIGHT)),
                    format!(
                        "lane {}, {}",
                        lane + 1,
                        timeline::timecode(frame_rate, start)
                    ),
                ))
            })
        })
        .collect()
}

#[component]
pub(crate) fn Timeline(state: VideoState) -> NodeId {
    let placed = create_signal(Rect::ZERO);
    create_effect(move || state.fit_timeline(placed.get().width().max(1.0)));

    let edit = state.edit;
    let rows = create_memo(move || {
        edit.with(|video| {
            timeline::lane_rows(video)
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
    });
    let lanes =
        create_memo(move || rows.get().iter().map(|row| row.lane + 1).max().unwrap_or(1) + 1);
    let ids = create_memo(move || rows.get().iter().map(|row| row.id).collect::<Vec<Uuid>>());

    let offset = create_signal(Vec2::ZERO);
    let camera = create_memo(move || Some(CanvasView::new(placed.get().min - offset.get(), 1.0)));

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

    // A clip carried along the timeline lands where `drop_target_at` says for the point under
    // the pointer; the marker draws what the site is over.
    let area = NodeRef::new();
    let site = use_drop_site(
        Func::new(|_: Uuid| true),
        Func::new(move |_: Uuid| {
            // The rows as they are at the lift, as every spec is read then: the board asks for
            // a place each frame, even while the clip is held still.
            let rows: Vec<ClipRow> = untrack(|| edit.with(timeline::lane_rows));
            drag::custom(area, move |at: Pos2, _| {
                let dragged = state.drag.get_untracked()?;
                untrack(|| {
                    edit.with(|video| {
                        let at = at + offset.get();
                        timeline::drop_target_at(
                            video,
                            &rows,
                            content.get(),
                            pixels_per_frame.get(),
                            at,
                            Some(&dragged),
                        )
                    })
                })
            })
            // From the keyboard the clip steps along a grid of frames on each lane, stopping
            // only where it would land: worked out once, at the first ask, in the timeline's
            // content, and placed where its pan shows them at each.
            .stepping({
                let found: Rc<RefCell<Option<Vec<drag::Stop>>>> = Rc::default();
                move |_| {
                    let mut found = found.borrow_mut();
                    let stops = found.get_or_insert_with(|| {
                        let Some(dragged) = state.drag.get_untracked() else {
                            return Vec::new();
                        };
                        untrack(|| {
                            edit.with(|video| {
                                keyboard_stops(
                                    video,
                                    &timeline::lane_rows(video),
                                    content.get(),
                                    pixels_per_frame.get(),
                                    (lanes.get(), state.frame_rate.get()),
                                    &dragged,
                                )
                            })
                        })
                    });
                    let offset = offset.get_untracked();
                    stops
                        .iter()
                        .map(|stop| drag::Stop {
                            rect: stop.rect.translate(-offset),
                            label: stop.label.clone(),
                        })
                        .collect()
                }
            })
        }),
        Callback::new(move |(_, target): (Uuid, TimelineDropTarget)| {
            let (Some(drag), Some(video)) = (state.drag.get_untracked(), state.video()) else {
                return;
            };
            let mut operations = Vec::new();
            timeline::apply_clip_drop(&video, drag.clip, target, &mut operations);
            for operation in operations {
                state.operate(operation);
            }
        }),
    );

    let target_drag = state.drag;
    let host_drag = state.editor().drag();
    let target = create_memo(move || {
        if target_drag.with(Option::is_some) {
            return site.over.get();
        }
        edit.with(|video| {
            let rows: Vec<ClipRow> = timeline::lane_rows(video);
            let content = content.get();
            let scale = pixels_per_frame.get();
            let dragged = host_drag
                .get()
                .filter(|drag| drag.block_id != state.block_id())?;
            let at =
                dragged.position - placed.get_untracked().min.to_vec2() + offset.get_untracked();
            timeline::drop_target_at(video, &rows, content, scale, at, None)
        })
    });

    // A keyboard carry is followed: the timeline pans to show the place it is over, and the
    // board aims it again at the stop it stepped to, now drawn elsewhere.
    create_effect(move || {
        let Some(over) = site.over.get() else {
            return;
        };
        untrack(|| {
            if !state.drag.get().is_some_and(|drag| drag.keyboard) {
                return;
            }
            let Some(marked) = marked(over, pixels_per_frame.get()) else {
                return;
            };
            let viewport = placed.get().size();
            let room = (content.get().size() - viewport).max(Vec2::ZERO);
            let at = offset.get();
            let next = Vec2::new(
                follow(at.x, viewport.x, marked.left(), marked.right()).clamp(0.0, room.x),
                follow(at.y, viewport.y, marked.top(), marked.bottom()).clamp(0.0, room.y),
            );
            if next != offset.get() {
                offset.set(next);
                beui::unstyled::drag_board().retrack();
            }
        });
    });

    let landing_drag = state.editor().drag();
    create_effect(move || {
        let dragged = landing_drag.get();
        untrack(|| land(state, target, dragged));
    });

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
    let playhead_x = create_memo(move || playhead.get() as f32 * pixels_per_frame.get());
    let playhead_height = create_memo(move || content.get().height());

    let built = view! {
        <Interactive
            @node_ref=&area
            cursor=CursorIcon::Default
            on_press={on_press}
            on_wheel={beui::unstyled::gestures::scroll_wheel(None, on_scroll)}
            @test_id={"video.timeline"}
        >
            <Canvas view={camera}>
                <ForEach value={lane_keys}>|lane: usize|
                    <LaneStrip lane content />
                </ForEach>
                <ForEach value={ticks}>|tick: Tick|
                    <RulerTick tick />
                </ForEach>
                <ForEach value={ids}>|id: Uuid| {
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
                        <Clip state id row selected scale={pixels_per_frame.read_only()} />
                    }
                }</ForEach>
                <DropMarker target={target} scale={pixels_per_frame.read_only()} />
                <CanvasItem x={playhead_x} y=0.0 width=1.5 height={playhead_height}>
                    <Frame fill=ColorRole::Accent />
                </CanvasItem>
            </Canvas>
        </Interactive>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placed_measured.get() {
            placed.set(measured.rect);
        }
    });
    built
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
        0 => ColorRole::Shade,
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
    let editor = state.editor();
    let element_test_id = format!("video.clip.{}", id);
    let grabbed_at = Rc::new(Cell::new(None::<Pos2>));
    let grabbing = Rc::clone(&grabbed_at);
    // Carried, the clip is moved by its own drag: how far into it it was grabbed sets where it
    // starts when it lands.
    let drag = Func::new(move |press: PointerPress| {
        let scale = scale.get_untracked().max(f32::EPSILON);
        // The keyboard lifts a clip by its start, so the stop it steps to is where it starts.
        let pressed = match press.pointer {
            PointerId::Keyboard => 0.0,
            _ => grabbed_at.get().map_or(0.0, |pos| pos.x),
        };
        state.begin_drag(Some(ClipDrag {
            clip: id,
            grab: (pressed / scale).max(0.0) as u64,
            keyboard: press.pointer == PointerId::Keyboard,
        }));
        Some(drag::Carry::new(id).preview(move || {
            view! {
                <Frame fill=ColorRole::Pressed radius=CLIP_RADIUS padding=THUMBNAIL_MARGIN>
                    <Text>{name}</Text>
                </Frame>
            }
        }))
    });
    let carried = is_carried(id);
    create_effect_on_change(
        move || carried.get(),
        move |carried, _| {
            if !*carried {
                state.begin_drag(None);
            }
        },
    );

    let face = view! {
                <Interactive on_press={move |press: PointerPress| grabbing.set(Some(press.local))}>
                    <Frame
                        fill={move || match attached.get() {
                            true => ColorRole::Hover,
                            false => ColorRole::Pressed,
                        }}
                        radius=CLIP_RADIUS
                        border={move || selected.get().then_some(Border::FOCUS)}
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
                            <ListItem grow=1.0>
                                <Text overflow=Overflow::Ellipsis>{name}</Text>
                            </ListItem>
                            <TrimHandle state={state} id={id} scale={scale} />
                        </List>
                    </Frame>
                </Interactive>
    };
    view! {
        <CanvasItem x={x} y={y} width={width} height=LANE_HEIGHT>
            <ClipControl state id named={element_test_id} name row drag>{face}</ClipControl>
        </CanvasItem>
    }
}

/// A clip on the timeline as a control: a click selects it, a drag carries it, and from the
/// keyboard it is a Tab stop, selected while it has the focus, with its focus ring, that Space
/// or Enter lifts and the arrows carry along the timeline. A screen reader reads it as a
/// button named for its block and where it is.
#[component]
fn ClipControl(
    state: VideoState,
    id: Uuid,
    named: String,
    name: Memo<String>,
    row: Memo<Lane>,
    drag: Func<PointerPress, Option<drag::Carry>>,
    children: Child,
) -> NodeId {
    let focused = create_signal(false);
    let rate = state.frame_rate;
    component_accessibility(create_memo(move || {
        let mut node = accesskit::Node::new(accesskit::Role::Button);
        let row = row.get();
        node.set_label(format!(
            "Clip {}, lane {}, {}",
            name.get(),
            row.lane + 1,
            timeline::timecode(rate.get(), row.start)
        ));
        node
    }));
    view! {
        <Interactive
            @test_id={named}
            focusable=true
            on_focus_change={move |focus: bool| {
                focused.set(focus);
                if focus {
                    state.select(Some(id));
                }
            }}
            cursor=CursorIcon::Grab
            drag={Some(drag)}
            on_click={move || state.select(Some(id))}
        >
            <FocusRing focused radius=CLIP_RADIUS>{children}</FocusRing>
        </Interactive>
    }
}


/// How far inside its edge the timeline keeps the place a keyboard carry is over.
const FOLLOW_MARGIN: f32 = 24.0;

/// Where a view `length` long, now at `at`, goes along one axis to show `near` to `far`
/// (as much of it from `near` as fits) `FOLLOW_MARGIN` inside its edges, moving no further
/// than it must.
fn follow(at: f32, length: f32, near: f32, far: f32) -> f32 {
    let far = far.min(near + length - FOLLOW_MARGIN * 2.0);
    if near < at + FOLLOW_MARGIN {
        near - FOLLOW_MARGIN
    } else if far > at + length - FOLLOW_MARGIN {
        far - length + FOLLOW_MARGIN
    } else {
        at
    }
}

/// Where the marker of `target` is drawn, in the timeline's content.
fn marked(target: TimelineDropTarget, scale: f32) -> Option<Rect> {
    match target {
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
}

#[component]
fn DropMarker(
    target: Memo<Option<TimelineDropTarget>>,
    scale: ReadSignal<f32>,
) -> DynamicSegment<CanvasItem> {
    let preview = create_memo(move || marked(target.get()?, scale.get()));
    view! {
        <Show value={preview}>|rect: ReadSignal<Rect>|
            <CanvasItem
                x={move || rect.get().left()}
                y={move || rect.get().top()}
                width={move || rect.get().width()}
                height={move || rect.get().height()}
                @test_id={"video.drop_marker"}
            >
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
