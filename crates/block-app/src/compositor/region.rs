use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{DragPoint, DropHandle, DropTarget};
use beui::{
    Align, CursorIcon, Forwarded, ForwardedInput, ImeCursor, Modifiers, NodeId, PointerButton,
    PointerEvent, PointerId, PointerPhase, Pos2, Rect, Region, Vec2, pos2,
};
use block_plugin_api::{ChildId, EditorInstanceId, EditorRegion, FrameSpec, PluginManifest};
use uuid::Uuid;

use crate::host::HostItem;
use crate::plugin_host::{
    self, BlockDrag, EditorView, HostChild, InstanceRole, Piece, RegionPlacement, RegionSlot,
    RegionView,
};
use crate::surfaces::HostItemFace;

#[derive(Clone, Debug, PartialEq)]
struct Failure {
    title: String,
    detail: String,
    restart: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RegionEditor {
    pub(crate) plugin: Arc<PluginManifest>,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block_types: Arc<crate::editors::PluginCatalog>,
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

pub(crate) type ChildView =
    Stored<Box<dyn Fn(ChildId, Memo<Option<HostChild>>, Memo<Rect>) -> NodeId>>;

#[derive(Clone)]
pub(crate) struct Occlusion(pub(crate) Memo<Vec<Rect>>);

#[derive(Clone, Copy)]
struct Presented(Memo<bool>);

#[derive(Clone, Copy)]
pub(crate) struct PanDrag {
    pub(crate) asked: ReadSignal<u64>,
    pub(crate) pan: Callback<Vec2>,
}

#[derive(Clone, Copy)]
struct Steering {
    pressed: Option<(PointerEvent, Pos2)>,
    panning: Option<PointerEvent>,
    rect: Rect,
}

impl Steering {
    const IDLE: Self = Self {
        pressed: None,
        panning: None,
        rect: Rect::ZERO,
    };

    fn steer(&mut self, input: ForwardedInput, pan: Callback<Vec2>) -> ForwardedInput {
        self.rect = input.rect;
        let mut events = Vec::with_capacity(input.events.len());
        for forwarded in input.events {
            let Forwarded::Pointer { event, .. } = &forwarded else {
                events.push(forwarded);
                continue;
            };
            let event = *event;
            if let Some(held) = self.panning.filter(|held| held.pointer == event.pointer) {
                match event.phase {
                    PointerPhase::Move => {
                        pan.call(event.pos - held.pos);
                        self.panning = Some(event);
                    }
                    PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => {
                        self.panning = None;
                    }
                    _ => {}
                }
                continue;
            }
            match event.phase {
                PointerPhase::Down(PointerButton::Primary) if self.pressed.is_none() => {
                    self.pressed = Some((event, event.pos));
                }
                PointerPhase::Move
                    if self
                        .pressed
                        .is_some_and(|(pressed, _)| pressed.pointer == event.pointer) =>
                {
                    self.pressed = self.pressed.map(|(pressed, _)| (pressed, event.pos));
                }
                PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone
                    if self
                        .pressed
                        .is_some_and(|(pressed, _)| pressed.pointer == event.pointer) =>
                {
                    self.pressed = None;
                }
                _ => {}
            }
            events.push(forwarded);
        }
        ForwardedInput { events, ..input }
    }

    fn take_over(&mut self, pan: Callback<Vec2>) -> Option<ForwardedInput> {
        let (pressed, at) = self.pressed.take()?;
        if at != pressed.pos {
            pan.call(at - pressed.pos);
        }
        let held = PointerEvent { pos: at, ..pressed };
        self.panning = Some(held);
        let cancel = match held.pointer {
            PointerId::Mouse => Forwarded::Left,
            _ => Forwarded::Pointer {
                event: PointerEvent {
                    phase: PointerPhase::Cancel,
                    ..held
                },
                over: None,
            },
        };
        Some(ForwardedInput {
            events: vec![cancel],
            rect: self.rect,
            hovered: false,
            focused: true,
            pointer: None,
            modifiers: held.modifiers,
        })
    }
}

pub(crate) fn occlusion() -> Option<Memo<Vec<Rect>>> {
    use_context::<Occlusion>().map(|Occlusion(rects)| rects)
}

fn next_mount() -> u64 {
    thread_local! {
        static MOUNTS: Cell<u64> = const { Cell::new(0) };
    }
    MOUNTS.with(|mounts| {
        let mount = mounts.get() + 1;
        mounts.set(mount);
        mount
    })
}

#[component]
pub(crate) fn PluginRegion(
    editor: RegionEditor,
    region: EditorRegion,
    #[prop(default = None)] frame: Prop<Option<FrameSpec>>,
    #[prop(default = None)] view: Prop<Option<EditorView>>,
    #[prop(default = 1.0)] zoom: Prop<f32>,
    #[prop(default = false)] passive: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    #[prop(default = None)] pan_drag: Option<PanDrag>,
    #[prop(default = false)] offscreen: bool,
    on_focus: Callback<()>,
    child_view: ChildView,
) -> NodeId {
    let instance = editor.instance;
    let place = super::desktop::place();
    let plugin_id = editor.plugin_id().to_owned();
    let floated_key = match region {
        EditorRegion::Window(window) => Some(super::desktop::FloatedKey {
            plugin: plugin_id.clone(),
            instance,
            window,
        }),
        _ => None,
    };
    let shell = super::shell();
    let shows_desktop = matches!(region, EditorRegion::Frame | EditorRegion::Display(_))
        && matches!(editor.role, InstanceRole::Editor(id, block) if id == be_block::LINUX_DESKTOP_EDITOR && shell.get_untracked() == Some(block.id));
    let hosts_windows = shows_desktop || floated_key.is_some();
    let primary = match (&floated_key, place) {
        (Some(key), Some(place)) => super::desktop::owns(place, key.clone()),
        _ => create_memo(|| true),
    };
    let floated_rect = floated_key.clone().map(super::desktop::rect_of);
    let desktop_editor = store(editor.clone());
    let name = editor.plugin.identity.name.clone();
    let crash_id = format!("{plugin_id}.{}.{region:?}.crashed", instance.0);
    let plugin_id = store(plugin_id);
    let block = editor.role.block().map(|block| block.id);
    let outer = use_context::<Presented>();
    let presented = create_memo(move || {
        region != EditorRegion::Preview && outer.is_some_and(|Presented(within)| within.get())
    });
    let any = super::any();
    let presenting = create_memo(move || {
        any.get();
        block.is_some_and(|block| {
            super::editors::editors().with(|open| {
                open.get(&block)
                    .is_some_and(crate::editors::PluginEditor::presenting_now)
            })
        })
    });
    provide_context(Presented(create_memo(move || {
        presented.get() || presenting.get()
    })));
    plugin_host::mount_region(RegionSlot {
        plugin: &editor.plugin,
        block_types: &editor.block_types,
        client_id: editor.client_id,
        role: editor.role,
        instance,
        region,
    });
    let mount = next_mount();
    on_cleanup(move || plugin_host::unmount_region(&plugin_id.value(), instance, region, mount));
    create_effect(move || {
        let presented = presented.get();
        untrack(|| plugin_host::set_presented(&plugin_id.value(), instance, presented));
    });
    on_cleanup(move || {
        if presented.get_untracked() {
            plugin_host::set_presented(&plugin_id.value(), instance, false);
        }
    });
    let revision = super::listen(&plugin_id.value());
    let placed = store(Cell::new(None::<EmbedPlacement>));
    let stored_key = store(floated_key.clone());
    let placer = Callback::new(move |placement: Option<EmbedPlacement>| match placement {
        Some(placement) => plugin_host::place_region(
            &plugin_id.value(),
            instance,
            region,
            mount,
            RegionPlacement {
                rect: placement.rect,
                clip: placement.clip,
                zoom: zoom.get_untracked(),
                raster: place.map(|place| place.scale.get_untracked()),
                windows: hosts_windows,
                placement: stored_key
                    .with(|key| key.as_ref().and_then(super::desktop::placement_of)),
            },
            frame.get_untracked(),
            view.get_untracked(),
        ),
        None => plugin_host::unplace_region(&plugin_id.value(), instance, region, mount),
    });
    let slot = EmbedSlot::new();
    slot.on_place(move |placement| {
        placed.with(|placed| placed.set(placement));
        placer.call(placement);
    });
    create_effect(move || {
        let _ = frame.get();
        let _ = view.get();
        let _ = zoom.get();
        let _ = place.map(|place| place.scale.get());
        let _ = floated_rect.map(|rect| rect.get());
        placer.call(placed.with(Cell::get));
    });
    if shows_desktop {
        let origin = create_memo(move || place.map_or(Pos2::ZERO, |place| place.display.get().min));
        create_effect(move || {
            let origin = origin.get();
            untrack(|| super::desktop::note_origin(&plugin_id.value(), instance, region, origin));
        });
        on_cleanup(move || super::desktop::forget_origin(&plugin_id.value(), instance, region));
    }
    let state = create_memo(move || {
        revision.get();
        plugin_host::region_view(
            &editor.plugin,
            instance,
            region,
            placed.with(Cell::get),
            view.get(),
        )
    });
    create_effect(move || {
        revision.get();
        let actions = plugin_host::take_region_actions(&plugin_id.value(), instance);
        if !actions.is_empty() {
            let from = super::editors::editors().block_of(&plugin_id.value(), instance);
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
        for (child, pick) in
            plugin_host::take_child_menu_picks(&plugin_id.value(), instance, &children)
        {
            if let Some((_, block)) = placed.iter().find(|(placed, _)| *placed == child) {
                super::editors::pick_frame_menu(*block, pick);
            }
        }
        for drag in plugin_host::take_block_drags(&plugin_id.value(), instance) {
            carry(drag);
        }
    });
    let rect = create_memo(move || state.with(|view| view.rect));
    let (reports, tick) = super::editors::reports();
    super::editors::provide_reports(reports);
    create_effect(move || {
        tick.get();
        let statuses = super::desktop::merged_statuses(
            (plugin_id.value().to_string(), instance, region),
            mount,
            Some(super::editors::statuses(&reports)),
        );
        plugin_host::report_children(&plugin_id.value(), instance, region, statuses);
    });
    on_cleanup(move || {
        super::desktop::merged_statuses(
            (plugin_id.value().to_string(), instance, region),
            mount,
            None,
        );
    });
    if offscreen {
        return view! {
            <Layers>
                <RegionChildren state={state} rect={rect} child_view below=true />
                <Embed slot={slot} punch=false />
                <RegionChildren state rect child_view below=false />
            </Layers>
        };
    }
    let frames = create_memo(move || {
        revision.get();
        plugin_host::frames(&plugin_id.value())
    });
    let drawing = |floating: bool| {
        let shown = create_memo(move || {
            let (pieces, loading, drawn, held, size) = state.with(|view| {
                let pieces = match floating {
                    true => view.floating.clone(),
                    false => view.base.clone(),
                };
                (
                    pieces,
                    view.loading,
                    view.drawn,
                    view.held,
                    view.rect.size(),
                )
            });
            (
                frames.get(),
                rotation.get(),
                opacity.get(),
                pieces,
                (loading, drawn, held, size),
            )
        });
        let held: Rc<RefCell<Option<(DrawingKey, beui::Drawing, u64)>>> =
            Rc::new(RefCell::new(None));
        let drawn = create_memo(move || {
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
                placed: plugin_host::region_placed(&plugin_id.value(), instance, region),
            };
            let previous = held.borrow().clone();
            let published = plugin_host::region_published(&plugin_id.value());
            if let Some((_, drawing, seen)) = previous.filter(|(was, _, _)| *was == key)
                && let Some(damage) = state.with_untracked(|view| {
                    plugin_host::region_damage(
                        &plugin_id.value(),
                        instance,
                        region,
                        view,
                        &pieces,
                        rotation,
                        seen,
                    )
                })
            {
                if damage.is_empty() {
                    *held.borrow_mut() = Some((key, drawing.clone(), published));
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
                *held.borrow_mut() = Some((key, redrawn.clone(), published));
                return Some(redrawn);
            }
            let drawing = state.with_untracked(|view| {
                plugin_host::region_drawing(
                    &plugin_id.value(),
                    instance,
                    region,
                    view,
                    &pieces,
                    rotation,
                    opacity,
                )
            });
            *held.borrow_mut() = drawing.clone().map(|drawing| (key, drawing, published));
            drawing
        });
        Prop::dynamic(move || draw_gpu(drawn.get()))
    };
    let base = drawing(false);
    let floating = drawing(true);
    let cursor = create_memo(move || state.with(|view| view.cursor.unwrap_or(CursorIcon::Default)));
    let handles_back = create_memo(move || state.with(|view| view.handles_back));
    let pastes = create_memo(move || state.with(|view| view.pastes.clone()));
    let clipboard_keys = create_memo(move || state.with(|view| view.clipboard_keys));
    let intercepted = create_memo(move || state.with(|view| view.intercepted_keys.clone()));
    let catcher = NodeRef::new();
    super::intercept::intercept_keys(plugin_id, instance, region, intercepted, move || {
        primary.get_untracked()
            && (hosts_windows
                || (region == EditorRegion::Frame
                    && block.is_some()
                    && shell.get_untracked() == block))
    });
    take_keyboard(state, &catcher, primary);
    focus_when_presenting(presenting, presented, &catcher, primary);
    toggle_keyboard_when_asked(state, primary);
    focus_when_asked(revision, plugin_id, instance, region, &catcher, primary);
    let catcher_zoom = zoom;
    create_effect(move || {
        revision.get();
        let zoom = catcher_zoom.get();
        if let Some(node) = catcher.try_get() {
            untrack(|| super::desktop::register_catcher(node, &plugin_id.value(), instance, region, zoom));
        }
    });
    on_cleanup(move || {
        if let Some(node) = catcher.try_get() {
            super::desktop::forget_catcher(node);
        }
    });
    let anchor = NodeRef::new();
    let ime = create_memo(move || state.with(|view| view.ime.is_some()));
    let ime_cursor = create_memo(move || {
        state.with(|view| {
            let area = view.ime.as_ref()?;
            Some(ImeCursor {
                node: anchor.try_get()?,
                rect: Some(area.cursor.translate(-view.rect.min.to_vec2())),
            })
        })
    });
    let ime_keyboard = create_memo(move || {
        state.with(|view| view.ime.as_ref().is_none_or(|area| area.keyboard.shown()))
    });
    let keyboard_asks = create_memo(move || {
        Some(
            state.with(|view| match view.ime.as_ref().map(|area| area.keyboard) {
                Some(beui::Keyboard::Shown { asks }) => asks,
                _ => 0,
            }),
        )
    });
    let ime_text = create_memo(move || {
        state.with(|view| view.ime.as_ref().and_then(|area| area.text.clone()))
    });
    let loading =
        create_memo(move || region == EditorRegion::Frame && state.with(|view| view.loading));
    let failure = create_memo(move || {
        state.with(|view| {
            view.error.as_ref().map(|(text, restart)| Failure {
                title: match restart {
                    true => format!("{name} stopped"),
                    false => format!("{name} could not start"),
                },
                detail: text.clone(),
                restart: restart.then(|| plugin_id.with(String::clone)),
            })
        })
    });
    let steering = StoredCell::new(Steering::IDLE);
    let forward = move |input: ForwardedInput| {
        let origin = place.map_or(beui::Vec2::ZERO, |place| {
            place.display.get_untracked().min.to_vec2()
        });
        if hosts_windows {
            super::desktop::follow_pointer(&input, origin);
        }
        let input = match stored_key.with(Clone::clone) {
            Some(key) => super::desktop::follow(&key, input, origin),
            None => input,
        };
        let input = match pan_drag {
            Some(pan_drag) => {
                let mut steered = steering.get();
                let input = steered.steer(input, pan_drag.pan);
                steering.set(steered);
                input
            }
            None => input,
        };
        if !input.events.is_empty() {
            let own = catcher.try_get();
            let rect = input.rect;
            let aim = move |over: Option<beui::Over>, pos: Pos2| {
                if let Some(over) = over {
                    if Some(over.node) == own {
                        return beui::Aim::Own;
                    }
                    if let Some((plugin, _, _, zoom)) = super::desktop::catcher(over.node)
                        && plugin_id.with(|own| *own == plugin)
                    {
                        return beui::Aim::Other {
                            node: over.node,
                            zoom,
                        };
                    }
                }
                match rect.contains(pos) {
                    true => beui::Aim::Own,
                    false => beui::Aim::Away,
                }
            };
            let target = |node: beui::NodeId| {
                super::desktop::catcher(node).map(|(_, instance, region, _)| (instance, region))
            };
            plugin_host::forward_region(&plugin_id.value(), instance, region, &input, &aim, &target);
        }
    };
    if let Some(pan_drag) = pan_drag {
        create_effect(move || {
            if pan_drag.asked.get() == 0 {
                return;
            }
            untrack(|| {
                let mut steered = steering.get();
                let cancel = steered.take_over(pan_drag.pan);
                steering.set(steered);
                if let Some(cancel) = cancel {
                    plugin_host::forward_region(
                        &plugin_id.value(),
                        instance,
                        region,
                        &cancel,
                        &|_, _| beui::Aim::Own,
                        &|_| None,
                    );
                }
            });
        });
    }
    let back = move |gesture: beui::BackGesture| {
        plugin_host::back_region(&plugin_id.value(), instance, region, gesture);
    };
    let occluded = occlusion();
    let beyond = move |local: Pos2| {
        let Some(placed) = frame.get_untracked().and_then(|frame| frame.content) else {
            return false;
        };
        region == EditorRegion::Frame
            && !placed.contains(local)
            && plugin_host::frame_rects(&plugin_id.value(), instance).is_some_and(|frame| {
                frame.content.contains(local)
                    && !frame
                        .floating
                        .iter()
                        .any(|floating| floating.contains(local))
            })
    };
    let takes = move |local: Pos2| {
        !passive.get_untracked()
            && !beyond(local)
            && state.with_untracked(|view| {
                let position = local + view.rect.min.to_vec2();
                view.takes(local)
                    && !occluded.as_ref().is_some_and(|occluded| {
                        occluded.with_untracked(|rects| {
                            rects.iter().any(|rect| rect.contains(position))
                        })
                    })
            })
    };
    let taken = move |point: DragPoint| {
        takes(point.pos - state.with_untracked(|view| view.rect.min.to_vec2()))
    };
    let over = move |over: Option<(BlockDrag, DragPoint)>| {
        let over = over
            .filter(|(_, point)| taken(*point))
            .map(|(drag, point)| (drag, point.pos));
        plugin_host::drag_over_region(&plugin_id.value(), instance, region, over, false);
    };
    let dropped = move |(drag, point): (BlockDrag, DragPoint)| {
        if taken(point) {
            plugin_host::drag_over_region(
                &plugin_id.value(),
                instance,
                region,
                Some((drag, point.pos)),
                true,
            );
        }
    };
    let pressed_outside = move |_: Pos2| {
        plugin_host::pressed_outside_region(&plugin_id.value(), instance, region);
    };
    let claims = move |(position, held): (Pos2, beui::Modifiers)| {
        !passive.get_untracked() && state.with_untracked(|view| view.claims(position, held))
    };
    let pen_surface = move |position: Pos2| {
        !passive.get_untracked() && state.with_untracked(|view| view.draws(position))
    };
    view! {
        <DropTarget on_over={over} on_drop={dropped}>|_: DropHandle|
            <Layers>
                <RegionChildren state={state} rect={rect} child_view below=true />
                <BackHandler enabled={handles_back} on_gesture={back}>
                    <Interactive
                        @node_ref={&catcher}
                        focusable=true
                        press_focus={create_memo(move || !presented.get())}
                        cursor={cursor}
                        ime={ime}
                        ime_keyboard={ime_keyboard}
                        keyboard_asks={keyboard_asks}
                        ime_cursor={ime_cursor}
                        ime_text={ime_text}
                        pastes={pastes}
                        clipboard_keys={clipboard_keys}
                        on_forward={forward}
                        hit_at={takes}
                        claim_at={claims}
                        drawing_at={pen_surface}
                        on_press_outside={pressed_outside}
                        on_focus_change={move |focused: bool| {
                            if focused {
                                on_focus.call(());
                            }
                        }}
                    >
                        <Embed slot={slot} punch=false @node_ref={&anchor}>
                            <Drawing draw={base} />
                        </Embed>
                    </Interactive>
                </BackHandler>
                <Notices loading failure id={crash_id.clone()} />
                <Show condition={shows_desktop}>|| {
                    view! {
                        <DesktopWindows editor={desktop_editor.with(Clone::clone)} state rect />
                    }
                }</Show>
                <RegionChildren state rect child_view below=false />
                <Drawing draw={floating} />
            </Layers>
        </DropTarget>
    }
}

#[component]
fn DesktopWindows(editor: RegionEditor, state: Memo<RegionView>, rect: Memo<Rect>) -> NodeId {
    let place = super::desktop::place().unwrap_or_else(|| {
        let display = create_memo(move || Rect::from_min_size(Pos2::ZERO, rect.get().size()));
        super::desktop::DesktopPlace {
            display,
            displays: create_memo(move || vec![display.get()]),
            scale: create_memo(|| crate::host::pixels_per_point()),
        }
    });
    provide_context(place);
    let inherited = occlusion();
    provide_context(Occlusion(create_memo(move || {
        let mut rects = inherited
            .as_ref()
            .map(|inherited| inherited.get())
            .unwrap_or_default();
        rects.extend(state.with(|view| view.floating_rects.clone()));
        rects
    })));
    let block = editor.role.block().map(|block| block.id);
    let keys = super::desktop::shown_on(place, editor.plugin_id().to_owned(), editor.instance);
    let editor = store(editor);
    view! {
        <Canvas>
            <ForEach keys={keys}>|window: u64| {
                let editor = editor.with(Clone::clone);
                let key = super::desktop::FloatedKey {
                    plugin: editor.plugin_id().to_owned(),
                    instance: editor.instance,
                    window,
                };
                let shown = super::desktop::rect_of(key);
                let local = create_memo(move || {
                    shown
                        .get()
                        .map_or(Rect::ZERO, |shown| shown.translate(-place.display.get().min.to_vec2()))
                });
                let x = create_memo(move || local.get().min.x);
                let y = create_memo(move || local.get().min.y);
                let width = create_memo(move || local.get().width());
                let height = create_memo(move || local.get().height());
                let region = EditorRegion::Window(window);
                let child_view: ChildView = match block {
                    Some(block) => super::editors::child_view(block, region),
                    None => store(Box::new(|_, _, _| view! {
                        <Frame />
                    })),
                };
                view! {
                    <CanvasItem x={x} y={y} width={width} height={height}>
                        <PluginRegion editor region child_view on_focus={Callback::new(|()| {})} />
                    </CanvasItem>
                }
            }</ForEach>
        </Canvas>
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
    let point = DragPoint::at(pointer.pos, Modifiers::NONE);
    board.begin(Rc::new(drag), point, Rc::new(|_| {}), Rc::new(|_| {}));
}

fn focus_when_asked(
    revision: beui::reactive::ReadSignal<u64>,
    plugin_id: Stored<String>,
    instance: EditorInstanceId,
    region: EditorRegion,
    catcher: &NodeRef,
    primary: Memo<bool>,
) {
    let catcher = *catcher;
    create_effect(move || {
        revision.get();
        untrack(|| {
            if !primary.get() {
                return;
            }
            if !plugin_host::take_focus_request(&plugin_id.value(), instance, region) {
                return;
            }
            if let Some(node) = catcher.try_get() {
                try_with_document(|document| document.focus_focusable(node));
            }
        });
    });
}

fn focus_when_presenting(
    presenting: Memo<bool>,
    presented: Memo<bool>,
    catcher: &NodeRef,
    primary: Memo<bool>,
) {
    let catcher = *catcher;
    let owns = create_memo(move || presenting.get() && !presented.get() && primary.get());
    create_effect(move || {
        if !owns.get() {
            return;
        }
        untrack(|| {
            if let Some(node) = catcher.try_get() {
                try_with_document(|document| document.focus_focusable(node));
            }
        });
    });
}

fn toggle_keyboard_when_asked(state: Memo<RegionView>, primary: Memo<bool>) {
    let toggles = create_memo(move || state.with(|view| view.keyboard_toggles));
    let seen = StoredCell::new(0u32);
    create_effect(move || {
        let toggles = toggles.get();
        if toggles != seen.get() {
            seen.set(toggles);
            if primary.get_untracked() {
                untrack(toggle_keyboard);
            }
        }
    });
}

fn take_keyboard(state: Memo<RegionView>, catcher: &NodeRef, primary: Memo<bool>) {
    let wants = create_memo(move || state.with(|view| view.wants_keyboard) && primary.get());
    let catcher = *catcher;
    create_effect(move || {
        let wanted = wants.get();
        untrack(|| {
            let Some(region) = catcher.try_get() else {
                return;
            };
            try_with_document(|document| match wanted {
                true if document.focused_node() != Some(region) => {
                    document.push_focus_scope(region);
                    document.focus_focusable(region);
                }
                true => {}
                false => document.pop_focus_scope(region),
            });
        });
    });
}

#[component]
fn Notices(loading: Memo<bool>, failure: Memo<Option<Failure>>, id: String) -> NodeId {
    let failed = create_memo(move || failure.get().is_some());
    let still_loading = create_memo(move || loading.get() && failure.get().is_none());
    let title = create_memo(move || {
        failure
            .get()
            .map(|failure| failure.title)
            .unwrap_or_default()
    });
    let detail = create_memo(move || {
        failure
            .get()
            .map(|failure| failure.detail)
            .unwrap_or_default()
    });
    let restart = create_memo(move || failure.get().and_then(|failure| failure.restart));
    let reloadable = create_memo(move || restart.get().is_some());
    let reload = move || {
        if let Some(plugin) = restart.get_untracked() {
            crate::host::push_command(crate::host::HostCommand::RestartPlugin(plugin));
        }
    };
    let loading_item = create_memo(|| HostItem::Notice {
        text: "Loading plugin…".to_owned(),
        spinner: true,
        icon: None,
    });
    view! {
        <Frame align=Align::Center>
            <List spacing=0.0>
                <Show condition={failed}>
                    <CrashNotice
                        title={title}
                        detail={detail}
                        reloadable={reloadable}
                        on_reload={reload}
                        id={id.clone()}
                    />
                </Show>
                <Show condition={still_loading}>
                    <HostItemFace content={loading_item} />
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
    let keys = create_memo(move || {
        state.with(|view| {
            view.children
                .iter()
                .filter(|child| (child.is_below() && !child.frame_owner) == below)
                .map(|child| child.child)
                .collect::<Vec<ChildId>>()
        })
    });
    view! {
        <Canvas>
            <ForEach keys={keys}>|key: ChildId| {
                let child = create_memo(move || {
                    state.with(|view| {
                        view.children.iter().find(|child| child.child == key).cloned()
                    })
                });
                let clip = create_memo(move || {
                    let origin = rect.get().min;
                    child
                        .get()
                        .map_or(Rect::ZERO, |child| child.clip.translate(-origin.to_vec2()))
                });
                let x = create_memo(move || clip.get().min.x);
                let y = create_memo(move || clip.get().min.y);
                let width = create_memo(move || clip.get().width());
                let height = create_memo(move || clip.get().height());
                let built = child_view.with(|view| view(key, child, rect));
                view! {
                    <CanvasItem x={x} y={y} width={width} height={height}>{built}</CanvasItem>
                }
            }</ForEach>
        </Canvas>
    }
}

#[cfg(test)]
mod tests;
