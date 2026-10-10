use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use beui::reactive::{
    BackHandler, Canvas, CanvasItem, Drawing, Embed, EmbedPlacement, EmbedSlot, ForEach, Frame,
    Interactive, Layers, List, Memo, NodeRef, Prop, Show, clone, component, create_effect,
    create_memo, draw_gpu, on_cleanup, try_with_document, untrack, use_context, use_screens, view,
};
use beui::unstyled::{DragPoint, DropHandle, DropTarget};
use beui::{
    Align, CursorIcon, ForwardedInput, ImeCursor, Modifiers, NodeId, Pos2, Rect, Region, Vec2, pos2,
};
use block_plugin_api::{ChildId, EditorInstanceId, EditorRegion, FrameSpec, PluginManifest};
use uuid::Uuid;

use crate::host::HostItem;
use crate::plugin_host::{
    self, BlockDrag, EditorView, HostChild, InstanceRole, Piece, RegionPlacement, RegionSlot,
    RegionView,
};
use crate::surfaces::HostItemFace;

#[derive(Clone)]
pub(crate) struct RegionEditor {
    pub(crate) plugin: Arc<PluginManifest>,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block_types: Arc<block_plugin_api::Catalog>,
    pub(crate) client_id: Uuid,
}

impl RegionEditor {
    pub(crate) fn plugin_id(&self) -> &str {
        &self.plugin.identity.id
    }
}

#[derive(Clone, PartialEq)]
struct DrawingKey {
    rotation: f32,
    opacity: f32,
    pieces: Vec<Piece>,
    drawn: Option<(u32, u32)>,
    held: Option<Rect>,
    size: Vec2,
    placed: Option<[u32; 3]>,
}

pub(crate) type ChildView = Rc<dyn Fn(ChildId, Memo<Option<HostChild>>, Memo<Rect>) -> NodeId>;

#[derive(Clone)]
pub(crate) struct Occlusion(pub(crate) Memo<Vec<Rect>>);

pub(crate) fn occlusion() -> Option<Memo<Vec<Rect>>> {
    use_context::<Occlusion>().map(|Occlusion(rects)| rects)
}

#[component]
pub(crate) fn PluginRegion(
    editor: RegionEditor,
    region: EditorRegion,
    #[prop(default = None)] frame: Prop<Option<FrameSpec>>,
    #[prop(default = None)] view: Prop<Option<EditorView>>,
    #[prop(default = false)] passive: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    child_view: ChildView,
) -> NodeId {
    let instance = editor.instance;
    let plugin_id = editor.plugin_id().to_owned();
    let block = editor.role.block().map(|block| block.id);
    plugin_host::mount_region(RegionSlot {
        plugin: &editor.plugin,
        block_types: &editor.block_types,
        client_id: editor.client_id,
        role: editor.role,
        instance,
        region,
    });
    on_cleanup(clone!(plugin_id -> move || {
        plugin_host::unmount_region(&plugin_id, instance, region)
    }));
    let revision = super::listen(&plugin_id);
    let placed: Rc<Cell<Option<EmbedPlacement>>> = Rc::new(Cell::new(None));
    let screens = use_screens();
    let place: Rc<dyn Fn(Option<EmbedPlacement>)> = Rc::new(
        clone!(plugin_id frame view screens -> move |placement: Option<EmbedPlacement>| {
            match placement {
                Some(placement) => plugin_host::place_region(
                    &plugin_id,
                    instance,
                    region,
                    RegionPlacement {
                        rect: placement.rect,
                        clip: placement.clip,
                        screens: screens.get_untracked(),
                    },
                    frame.peek(),
                    view.peek(),
                ),
                None => plugin_host::unplace_region(&plugin_id, instance, region),
            }
        }),
    );
    let slot = EmbedSlot::new();
    slot.on_place(clone!(placed place -> move |placement| {
        placed.set(placement);
        place(placement);
    }));
    create_effect(clone!(placed place frame view -> move || {
        let _ = frame.get();
        let _ = view.get();
        screens.with(|_| ());
        place(placed.get());
    }));
    let state = create_memo(clone!(revision -> move || {
        revision.get();
        plugin_host::region_view(&editor.plugin, instance, region)
    }));
    create_effect(clone!(revision plugin_id state -> move || {
        revision.get();
        let actions = plugin_host::take_region_actions(&plugin_id, instance);
        if !actions.is_empty() {
            let from = super::editors::editors().block_of(&plugin_id, instance);
            for action in actions {
                super::act(from, action);
            }
        }
        let placed: Vec<(ChildId, Uuid)> = state.with_untracked(|view| {
            view.children
                .iter()
                .filter_map(|child| Some((child.child, child.block_id()?)))
                .collect()
        });
        let children: Vec<ChildId> = placed.iter().map(|(child, _)| *child).collect();
        for (child, pick) in plugin_host::take_child_menu_picks(&plugin_id, instance, &children) {
            if let Some((_, block)) = placed.iter().find(|(placed, _)| *placed == child) {
                super::editors::pick_frame_menu(*block, pick);
            }
        }
        for drag in plugin_host::take_block_drags(&plugin_id, instance) {
            carry(drag);
        }
    }));
    let frames = create_memo(clone!(revision plugin_id -> move || {
        revision.get();
        plugin_host::frames(&plugin_id)
    }));
    let drawing = |floating: bool| {
        let shown = create_memo(clone!(state frames rotation opacity -> move || {
            let (pieces, loading, drawn, held, size) = state.with(|view| {
                let pieces = match floating {
                    true => view.floating.clone(),
                    false => view.base.clone(),
                };
                (pieces, view.loading, view.drawn, view.held, view.rect.size())
            });
            (
                frames.get(),
                rotation.get(),
                opacity.get(),
                pieces,
                (loading, drawn, held, size),
            )
        }));
        let held: Rc<RefCell<Option<(DrawingKey, beui::Drawing)>>> = Rc::new(RefCell::new(None));
        let drawn = create_memo(clone!(plugin_id state -> move || {
            let (_, rotation, opacity, pieces, _) = shown.get();
            let (drawn, held_rect, size) =
                state.with_untracked(|view| (view.drawn, view.held, view.rect.size()));
            let key = DrawingKey {
                rotation,
                opacity,
                pieces: pieces.clone(),
                drawn,
                held: held_rect,
                size,
                placed: plugin_host::region_placed(&plugin_id, instance, region),
            };
            let previous = held.borrow().clone();
            if let Some((_, drawing)) = previous.filter(|(was, _)| *was == key)
                && let Some(damage) = state.with_untracked(|view| {
                    plugin_host::region_damage(&plugin_id, instance, region, view, &pieces, rotation)
                })
            {
                if damage.is_empty() {
                    return Some(drawing);
                }
                let damaged = damage.iter().fold(Region::default(), |damaged, fraction| {
                    damaged.union(Region::from(
                        Rect::from_min_max(
                            pos2(fraction.min.x * size.x, fraction.min.y * size.y),
                            pos2(fraction.max.x * size.x, fraction.max.y * size.y),
                        )
                        .expand(1.0),
                    ))
                });
                let redrawn = drawing.redrawn(damaged);
                *held.borrow_mut() = Some((key, redrawn.clone()));
                return Some(redrawn);
            }
            let drawing = state.with_untracked(|view| {
                plugin_host::region_drawing(
                    &plugin_id, instance, region, view, &pieces, rotation, opacity,
                )
            });
            *held.borrow_mut() = drawing.clone().map(|drawing| (key, drawing));
            drawing
        }));
        Prop::Dynamic(Rc::new(move || draw_gpu(drawn.get())))
    };
    let base = drawing(false);
    let floating = drawing(true);
    let cursor = create_memo(clone!(state -> move || {
        state.with(|view| view.cursor.unwrap_or(CursorIcon::Default))
    }));
    let handles_back = create_memo(clone!(state -> move || state.with(|view| view.handles_back)));
    let intercepted = create_memo(clone!(state -> move || {
        state.with(|view| view.intercepted_keys.clone())
    }));
    let shell = super::shell();
    super::intercept::intercept_keys(
        plugin_id.clone(),
        instance,
        region,
        intercepted,
        move || region == EditorRegion::Frame && block.is_some() && shell.get_untracked() == block,
    );
    let catcher = NodeRef::new();
    take_keyboard(&state, &catcher);
    let anchor = NodeRef::new();
    let ime = create_memo(clone!(state -> move || state.with(|view| view.ime.is_some())));
    let ime_cursor = create_memo(clone!(state anchor -> move || {
        state.with(|view| {
            let area = view.ime.as_ref()?;
            Some(ImeCursor {
                node: anchor.try_get()?,
                rect: Some(area.cursor.translate(-view.rect.min.to_vec2())),
            })
        })
    }));
    let ime_keyboard = create_memo(clone!(state -> move || {
        state.with(|view| view.ime.as_ref().is_none_or(|area| area.keyboard))
    }));
    let ime_text = create_memo(clone!(state -> move || {
        state.with(|view| view.ime.as_ref().and_then(|area| area.text.clone()))
    }));
    let loading = create_memo(clone!(state -> move || {
        region == EditorRegion::Frame && state.with(|view| view.loading)
    }));
    let failure = create_memo(clone!(state plugin_id -> move || {
        state.with(|view| {
            view.error.as_ref().map(|(text, restart)| HostItem::Error {
                text: text.clone(),
                restart: restart.then(|| plugin_id.clone()),
            })
        })
    }));
    let rect = create_memo(clone!(state -> move || state.with(|view| view.rect)));
    let (reports, tick) = super::editors::reports();
    super::editors::provide_reports(reports.clone());
    create_effect(clone!(plugin_id -> move || {
        tick.get();
        plugin_host::report_children(&plugin_id, instance, region, super::editors::statuses(&reports));
    }));
    let forward = clone!(plugin_id -> move |input: ForwardedInput| {
        plugin_host::forward_region(&plugin_id, instance, region, &input);
    });
    let back = clone!(plugin_id -> move |gesture: beui::BackGesture| {
        plugin_host::back_region(&plugin_id, instance, region, gesture);
    });
    let occluded = occlusion();
    let takes: Rc<dyn Fn(Pos2) -> bool> = Rc::new(clone!(state passive -> move |local: Pos2| {
        !passive.peek()
            && state.with_untracked(|view| {
                let position = local + view.rect.min.to_vec2();
                view.takes(local)
                    && !occluded.as_ref().is_some_and(|occluded| {
                        occluded.with_untracked(|rects| rects.iter().any(|rect| rect.contains(position)))
                    })
            })
    }));
    let taken = clone!(state takes -> move |point: DragPoint| {
        takes(point.pos - state.with_untracked(|view| view.rect.min.to_vec2()))
    });
    let over = clone!(plugin_id taken -> move |over: Option<(BlockDrag, DragPoint)>| {
        let over = over
            .filter(|(_, point)| taken(*point))
            .map(|(drag, point)| (drag, point.pos));
        plugin_host::drag_over_region(&plugin_id, instance, region, over, false);
    });
    let dropped = clone!(plugin_id -> move |(drag, point): (BlockDrag, DragPoint)| {
        if taken(point) {
            plugin_host::drag_over_region(&plugin_id, instance, region, Some((drag, point.pos)), true);
        }
    });
    let pressed_outside = clone!(plugin_id -> move |_: Pos2| {
        plugin_host::pressed_outside_region(&plugin_id, instance, region);
    });
    let takes = move |local: Pos2| takes(local);
    let claims = clone!(state passive -> move |(position, held): (Pos2, beui::Modifiers)| {
        !passive.peek() && state.with_untracked(|view| view.claims(position, held))
    });
    let below = Rc::clone(&child_view);
    let above = child_view;
    view! {
        <DropTarget on_over={over} on_drop={dropped}>
            {move |_: DropHandle| view! {
                <Layers>
                    <RegionChildren
                        state={state.clone()}
                        rect={rect.clone()}
                        child_view={below}
                        below=true
                    />
                    <BackHandler enabled={handles_back} on_gesture={back}>
                        <Interactive
                            @node_ref={&catcher}
                            focusable=true
                            cursor={cursor}
                            ime={ime}
                            ime_keyboard={ime_keyboard}
                            ime_cursor={ime_cursor}
                            ime_text={ime_text}
                            on_forward={forward}
                            forward_at={takes}
                            claim_at={claims}
                            on_press_outside={pressed_outside}
                        >
                            <Embed slot={slot} punch=false @node_ref={&anchor}>
                                <Drawing draw={base} />
                            </Embed>
                        </Interactive>
                    </BackHandler>
                    <Notices loading failure />
                    <RegionChildren state rect child_view={above} below=false />
                    <Drawing draw={floating} />
                </Layers>
            }}
        </DropTarget>
    }
}

fn carry(drag: BlockDrag) {
    let Some((board, pointer)) =
        try_with_document(|document| (document.drag_board(), document.last_pointer))
    else {
        return;
    };
    let Some(pointer) = pointer.filter(|_| !board.carrying()) else {
        return;
    };
    let point = DragPoint {
        pos: pointer.pos,
        modifiers: Modifiers::NONE,
    };
    board.begin(Rc::new(drag), point, Rc::new(|_| {}));
}

fn take_keyboard(state: &Memo<RegionView>, catcher: &NodeRef) {
    let wants = create_memo(clone!(state -> move || state.with(|view| view.wants_keyboard)));
    let before = Cell::new(None::<NodeId>);
    let catcher = catcher.clone();
    create_effect(move || {
        let wanted = wants.get();
        untrack(|| {
            let Some(region) = catcher.try_get() else {
                return;
            };
            try_with_document(|document| {
                let focused = document.focused_node();
                match wanted {
                    true if focused != Some(region) => {
                        before.set(focused);
                        document.focus_focusable(region);
                    }
                    true => {}
                    false => {
                        let back = before.take().filter(|back| document.contains(*back));
                        if let Some(back) = back
                            && focused == Some(region)
                        {
                            document.focus_focusable(back);
                        }
                    }
                }
            });
        });
    });
}

#[component]
fn Notices(loading: Memo<bool>, failure: Memo<Option<HostItem>>) -> NodeId {
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let still_loading = create_memo(clone!(failure loading -> move || {
        loading.get() && failure.get().is_none()
    }));
    let failure_item = create_memo(move || {
        failure.get().unwrap_or(HostItem::Error {
            text: String::new(),
            restart: None,
        })
    });
    let loading_item = create_memo(|| HostItem::Notice {
        text: "Loading plugin…".to_owned(),
        spinner: true,
    });
    view! {
        <Frame align_horizontal=Align::Center align_vertical=Align::Center>
            <List spacing=0.0>
                <Show condition={failed}>
                    <HostItemFace content={failure_item.clone()} />
                </Show>
                <Show condition={still_loading}>
                    <HostItemFace content={loading_item.clone()} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn RegionChildren(
    state: Memo<RegionView>,
    rect: Memo<Rect>,
    child_view: ChildView,
    below: bool,
) -> NodeId {
    let keys = create_memo(clone!(state -> move || {
        state.with(|view| {
            view.children
                .iter()
                .filter(|child| (child.is_below() && !child.frame_owner) == below)
                .map(|child| child.child)
                .collect::<Vec<ChildId>>()
        })
    }));
    view! {
        <Canvas>
            <ForEach keys={keys}>
                {move |key: ChildId| {
                    let child = create_memo(clone!(state -> move || {
                        state.with(|view| {
                            view.children.iter().find(|child| child.child == key).cloned()
                        })
                    }));
                    let clip = create_memo(clone!(child rect -> move || {
                        let origin = rect.get().min;
                        child
                            .get()
                            .map_or(Rect::ZERO, |child| child.clip.translate(-origin.to_vec2()))
                    }));
                    let x = create_memo(clone!(clip -> move || clip.get().min.x));
                    let y = create_memo(clone!(clip -> move || clip.get().min.y));
                    let width = create_memo(clone!(clip -> move || clip.get().width()));
                    let height = create_memo(clone!(clip -> move || clip.get().height()));
                    let built = child_view(key, child, rect.clone());
                    view! {
                        <CanvasItem x={x} y={y} width={width} height={height}>{built}</CanvasItem>
                    }
                }}
            </ForEach>
        </Canvas>
    }
}
