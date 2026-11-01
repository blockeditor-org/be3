use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};

use beui::native::try_with_document;
use beui::native::{Embed, EmbedPlacement, EmbedSlot};
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Align, CursorIcon, Dismiss, Forwarded, ForwardedInput, ImeCursor, LayerPolicy, Modifiers,
    NodeId, OverlayAnchor, Placement, PointerButton, PointerEvent, PointerId, PointerPhase, Pos2,
    Rect, Vec2, WindowId, WindowPlacement, WindowReport, native::Region, pos2,
};
use block_plugin_api::{
    ChildId, EditorInstanceId, FrameSpec, HostView, ParentView, View, ViewId, ViewPlacement,
};
use block_plugin_manifest::PluginManifest;
use uuid::Uuid;

use super::anchored::AnchoredViews;
use crate::surfaces::CrashNotice;
use beui::native::draw_gpu;
use beui::reactive::toggle_keyboard;
use block_app_desktop::host_item::HostItem;
use block_app_desktop::host_item::HostItemFace;
use block_app_desktop::occlusion::{Occlusion, occlusion};
use block_app_plugin_host::{
    self, BlockDrag, EditorView, HostChild, InstanceRole, Piece, PlacedView, ViewPlace, ViewSlot,
};

mod handoff;

#[derive(Clone, Debug, PartialEq)]
struct Failure {
    title: String,
    detail: String,
    restart: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ViewEditor {
    pub(crate) plugin: Arc<PluginManifest>,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block_types: Arc<block_app_plugin_host::PluginCatalog>,
    pub(crate) client_id: Uuid,
}

impl ViewEditor {
    pub(crate) fn plugin_id(&self) -> &str {
        &self.plugin.identity.id
    }
}

#[derive(Clone, PartialEq)]
struct DrawingKey {
    rotation: f32,
    opacity: f32,
    corner_radius: f32,
    pieces: Vec<Piece>,
    drawn: Option<(u32, u32)>,
    held: Option<Rect>,
    size: Vec2,
    placed: Option<[u32; 3]>,
}

pub(crate) type ChildView =
    Stored<Box<dyn Fn(ChildId, Memo<Option<HostChild>>, Memo<Rect>) -> NodeId>>;

#[derive(Clone, Copy)]
struct Presented(Memo<bool>);

#[derive(Clone, Copy)]
struct Placer(Stored<String>);

#[derive(Clone, Copy)]
struct Enclosing(Memo<Option<NodeRef>>);

#[derive(Clone, Copy)]
pub(crate) struct GivenKeyboard(pub(crate) Memo<bool>);

#[derive(Clone, Copy)]
pub(crate) struct Rounded(pub(crate) Memo<f32>);

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
    focused: bool,
}

impl Steering {
    const IDLE: Self = Self {
        pressed: None,
        panning: None,
        rect: Rect::ZERO,
        focused: false,
    };

    fn steer(&mut self, input: ForwardedInput, pan: Callback<Vec2>) -> ForwardedInput {
        self.rect = input.rect;
        self.focused = input.focused;
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
            focused: self.focused,
            pointer: None,
            modifiers: held.modifiers,
        })
    }
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
pub(crate) fn PluginView(
    editor: ViewEditor,
    region: View,
    #[prop(default = None)] frame: Prop<Option<FrameSpec>>,
    #[prop(default = None)] view: Prop<Option<EditorView>>,
    #[prop(default = 1.0)] zoom: Prop<f32>,
    #[prop(default = false)] passive: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    #[prop(default = None)] pan_drag: Option<PanDrag>,
    #[prop(default = false)] offscreen: bool,
    #[prop(default = false)] nests_windows: bool,
    #[prop(default = None)] opens_at: Option<Memo<Rect>>,
    #[prop(default = None)] shares: Option<ChildNodes>,
    on_focus: Callback<()>,
    child_view: ChildView,
) -> NodeId {
    let instance = editor.instance;
    let place = super::desktop::place();
    let windowing = ((matches!(
        region.placement,
        ViewPlacement::Parent(ParentView::Main) | ViewPlacement::Dock
    ) && !offscreen)
        || nests_windows)
        .then(|| editor.clone());
    let plugin_id = editor.plugin_id().to_owned();
    let floated_key = region.is_window().then(|| super::desktop::FloatedKey {
        plugin: plugin_id.clone(),
        instance,
        window: region.id,
    });
    let shell = super::shell();
    let shows_desktop = matches!(
        region.placement,
        ViewPlacement::Parent(ParentView::Main) | ViewPlacement::Host(HostView::Display(_))
    ) && matches!(editor.role, InstanceRole::Editor(id, block) if id == be_block::DESKTOP_EDITOR && shell.get_untracked() == Some(block.id));
    let hosts_windows = shows_desktop || floated_key.is_some();
    let primary = match (&floated_key, place) {
        (Some(key), Some(place)) => super::desktop::owns(place, key.clone()),
        _ => create_memo(|| true),
    };
    let floated_rect = floated_key.clone().map(super::desktop::rect_of);
    let floated_placement = floated_key.clone().map(super::desktop::placement_memo);
    let floated_origin = floated_key.clone().map(super::desktop::origin_memo);
    let origin = create_memo(move || match floated_origin {
        Some(origin) => origin.get(),
        None if shows_desktop => Some(place.map_or(Pos2::ZERO, |place| place.display.get().min)),
        None => None,
    });
    let desktop_editor = store(editor.clone());
    let name = editor.plugin.identity.name.clone();
    let crash_id = format!("{plugin_id}.{}.{}.crashed", instance.0, region.id.0);
    let plugin_id = store(plugin_id);
    let block = editor.role.block().map(|block| block.id);
    let outer = use_context::<Presented>();
    let presented = create_memo(move || {
        !region.is_preview() && outer.is_some_and(|Presented(within)| within.get())
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
    let placer = use_context::<Placer>().map(|Placer(placer)| placer.value().to_string());
    provide_context(Placer(plugin_id));
    block_app_plugin_host::mount_view(ViewSlot {
        plugin: &editor.plugin,
        placer: placer.as_deref(),
        block_types: &editor.block_types,
        client_id: editor.client_id,
        role: editor.role,
        instance,
        view: region,
    });
    let mount = next_mount();
    super::desktop::show_view(&plugin_id.value(), instance, region);
    on_cleanup(move || {
        block_app_plugin_host::unmount_view(&plugin_id.value(), instance, region.id, mount);
        super::desktop::hide_view(&plugin_id.value(), instance, region.id);
    });
    create_effect(move || {
        let presented = presented.get();
        untrack(|| block_app_plugin_host::set_presented(&plugin_id.value(), instance, presented));
    });
    on_cleanup(move || {
        if presented.get_untracked() {
            block_app_plugin_host::set_presented(&plugin_id.value(), instance, false);
        }
    });
    let corner_radius = use_context::<Rounded>().map(|Rounded(radius)| radius);
    provide_context(Rounded(create_memo(|| 0.0)));
    let revision = super::listen(&plugin_id.value());
    let placed = store(Cell::new(None::<EmbedPlacement>));
    let window = super::anchored::host_window();
    let parent_bounds = use_context::<super::anchored::AnchorBounds>();
    let viewport = use_viewport_size();
    let anchor_bounds = create_memo(move || {
        let parent = parent_bounds.map_or_else(
            || Rect::from_min_size(Pos2::ZERO, viewport.get()),
            |super::anchored::AnchorBounds(bounds)| bounds.get(),
        );
        super::anchored::bounds_within(parent, parent, false)
    });
    provide_context(super::anchored::AnchorBounds(anchor_bounds));
    let stored_key = store(floated_key.clone());
    let windowed = windowing.is_some();
    let placer = Callback::new(move |placement: Option<EmbedPlacement>| match placement {
        Some(placement) => block_app_plugin_host::place_view(
            &plugin_id.value(),
            instance,
            region.id,
            mount,
            ViewPlace {
                rect: placement.rect,
                clip: placement.clip,
                zoom: zoom.get_untracked(),
                raster: placement.raster,
                windows: hosts_windows || windowed,
                placement: stored_key
                    .with(|key| key.as_ref().and_then(super::desktop::placement_of)),
                origin: origin.get_untracked(),
                anchor_bounds: (!offscreen && !region.is_preview())
                    .then(|| anchor_bounds.get_untracked()),
            },
            frame.get_untracked(),
            view.get_untracked(),
        ),
        None => block_app_plugin_host::unplace_view(&plugin_id.value(), instance, region.id, mount),
    });
    let slot = EmbedSlot::new();
    slot.on_place(move |placement| {
        placed.with(|placed| placed.set(placement));
        placer.call(placement);
    });
    if let Some(at) = opens_at.map(|at| at.get_untracked())
        && at.is_positive()
    {
        let placement = EmbedPlacement {
            rect: at,
            clip: at,
            radius: 0.0,
            raster: 1.0,
        };
        placed.with(|placed| placed.set(Some(placement)));
        placer.call(Some(placement));
    }
    create_effect(move || {
        let _ = frame.get();
        let _ = view.get();
        let _ = zoom.get();
        let _ = floated_rect.map(|rect| rect.get());
        let _ = floated_placement.map(|placement| placement.get());
        let _ = origin.get();
        let _ = anchor_bounds.get();
        placer.call(placed.with(Cell::get));
    });
    if shows_desktop {
        create_effect(move || {
            let origin = origin.get().unwrap_or(Pos2::ZERO);
            untrack(|| {
                super::desktop::note_origin(&plugin_id.value(), instance, region.id, origin)
            });
        });
        on_cleanup(move || super::desktop::forget_origin(&plugin_id.value(), instance, region.id));
    }
    let state = create_memo(move || {
        revision.get();
        block_app_plugin_host::placed_view(
            &editor.plugin,
            instance,
            region.id,
            placed.with(Cell::get),
            view.get(),
        )
    });
    if shows_desktop {
        let key = (plugin_id.value().to_string(), instance, region.id);
        let held = key.clone();
        create_effect(move || {
            let origin = origin.get().unwrap_or(Pos2::ZERO);
            let area = state.with(|view| view.work_area);
            let area = area.map(|area| area.translate(origin.to_vec2()));
            untrack(|| super::desktop::note_work_area(held.clone(), area));
        });
        on_cleanup(move || super::desktop::note_work_area(key, None));
    }
    create_effect(move || {
        revision.get();
        let actions = block_app_plugin_host::take_view_actions(&plugin_id.value(), instance);
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
        for (child, pick, anchor) in
            block_app_plugin_host::take_child_menu_picks(&plugin_id.value(), instance, &children)
        {
            if let Some((_, block)) = placed.iter().find(|(placed, _)| *placed == child) {
                super::editors::pick_frame_menu(*block, pick, anchor);
            }
        }
        for drag in block_app_plugin_host::take_block_drags(&plugin_id.value(), instance) {
            carry(drag);
        }
    });
    let rect = create_memo(move || state.with(|view| view.rect));
    let catcher = NodeRef::new();
    let enclosing = use_context::<Enclosing>();
    let encloses = match (offscreen, enclosing) {
        (false, _) => create_memo(move || Some(catcher)),
        (true, Some(Enclosing(enclosing))) => enclosing,
        (true, None) => create_memo(|| None),
    };
    let anchors = super::anchored::anchor_host();
    let children = ChildNodes::joining(
        shares,
        Joined {
            mount,
            state,
            rect,
            child_view,
            anchors,
            encloses,
        },
    );
    let (reports, tick) = super::editors::reports();
    super::editors::provide_reports(reports);
    create_effect(move || {
        tick.get();
        let statuses = super::desktop::merged_statuses(
            (plugin_id.value().to_string(), instance, region.id),
            mount,
            Some(super::editors::statuses(&reports)),
        );
        block_app_plugin_host::report_children(&plugin_id.value(), instance, region.id, statuses);
    });
    on_cleanup(move || {
        super::desktop::merged_statuses(
            (plugin_id.value().to_string(), instance, region.id),
            mount,
            None,
        );
    });
    if offscreen {
        return view! {
            <Layers>
                <ViewChildren state={state} rect={rect} nodes={children} below=true />
                <Embed slot={slot} punch=false />
                <ViewChildren state rect nodes={children} below=false />
            </Layers>
        };
    }
    let frames = create_memo(move || {
        revision.get();
        block_app_plugin_host::frames(&plugin_id.value())
    });
    let placed = create_memo(move || {
        revision.get();
        block_app_plugin_host::view_placed(&plugin_id.value(), instance, region.id)
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
                (
                    rotation.get(),
                    corner_radius.map_or(0.0, |radius| radius.get()),
                ),
                opacity.get(),
                pieces,
                (loading, drawn, held, size),
                placed.get(),
            )
        });
        let held: Rc<
            RefCell<
                Option<(
                    DrawingKey,
                    beui::native::Drawing,
                    u64,
                    block_app_plugin_host::Pieces,
                )>,
            >,
        > = Rc::new(RefCell::new(None));
        let drawn = create_memo(move || {
            let (_, (rotation, corner_radius), opacity, pieces, _, placed) = shown.get();
            let (drawn, held_rect, size) =
                state.with_untracked(|view| (view.drawn, view.held, view.rect.size()));
            let key = DrawingKey {
                rotation,
                opacity,
                corner_radius,
                pieces: pieces.clone(),
                drawn,
                held: held_rect,
                size,
                placed,
            };
            let previous = held.borrow().clone();
            let published = block_app_plugin_host::view_published(&plugin_id.value());
            let reused = previous.and_then(|(was, drawing, seen, shared)| {
                let repieced = DrawingKey {
                    pieces: pieces.clone(),
                    ..was.clone()
                };
                if repieced != key {
                    return None;
                }
                let uncovered = match was.pieces == pieces {
                    true => Vec::new(),
                    false => block_app_plugin_host::uncovered_pieces(&was.pieces, &pieces)?,
                };
                let damage = state.with_untracked(|view| {
                    block_app_plugin_host::view_damage(
                        &plugin_id.value(),
                        instance,
                        region.id,
                        view,
                        &pieces,
                        rotation,
                        seen,
                    )
                })?;
                Some((drawing, shared, [damage, uncovered].concat()))
            });
            if let Some((drawing, shared, damage)) = reused {
                *shared.borrow_mut() = pieces;
                if damage.is_empty() {
                    *held.borrow_mut() = Some((key, drawing.clone(), published, shared));
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
                *held.borrow_mut() = Some((key, redrawn.clone(), published, shared));
                return Some(redrawn);
            }
            let drawing = state.with_untracked(|view| {
                block_app_plugin_host::view_drawing(
                    &plugin_id.value(),
                    instance,
                    region.id,
                    view,
                    &pieces,
                    rotation,
                    opacity,
                    corner_radius,
                )
            });
            *held.borrow_mut() = drawing
                .clone()
                .map(|(drawing, shared)| (key, drawing, published, shared));
            drawing.map(|(drawing, _)| drawing)
        });
        Prop::dynamic(move || draw_gpu(drawn.get()))
    };
    let base = drawing(false);
    let floating = drawing(true);
    let cursor = create_memo(move || state.with(|view| view.cursor.unwrap_or(CursorIcon::Default)));
    let handles_back = create_memo(move || state.with(|view| view.handles_back));
    let pastes = create_memo(move || state.with(|view| view.pastes.clone()));
    let clipboard_keys = create_memo(move || state.with(|view| view.clipboard_keys));
    let every_sample = create_memo(move || state.with(|view| view.every_sample));
    let intercepted = create_memo(move || state.with(|view| view.intercepted_keys.clone()));
    provide_context(Enclosing(encloses));
    let given = use_context::<GivenKeyboard>().map(|GivenKeyboard(given)| given);
    provide_context(GivenKeyboard(create_memo(|| false)));
    super::intercept::intercept_keys(plugin_id, instance, region.id, intercepted, move || {
        primary.get_untracked()
            && (hosts_windows
                || (region.is_main() && block.is_some() && shell.get_untracked() == block))
    });
    take_keyboard(state, given, &catcher, primary);
    focus_when_presenting(presenting, presented, &catcher, primary);
    toggle_keyboard_when_asked(state, primary);
    focus_when_asked(revision, plugin_id, instance, region, &catcher, primary);
    let catcher_zoom = zoom;
    create_effect(move || {
        revision.get();
        let zoom = catcher_zoom.get();
        let rect = state.with(|view| view.rect);
        if let Some(node) = catcher.try_get() {
            untrack(|| {
                super::desktop::register_catcher(
                    node,
                    super::desktop::Catcher {
                        plugin: plugin_id.value().to_string(),
                        instance,
                        region: region.id,
                        zoom,
                        rect,
                        parent: enclosing
                            .and_then(|Enclosing(parent)| parent.get())
                            .and_then(|parent| parent.try_get()),
                    },
                )
            });
        }
    });
    on_cleanup(move || {
        if let Some(node) = catcher.try_get() {
            super::desktop::forget_catcher(node);
        }
    });
    super::anchored::register(super::anchored::AnchorOwner {
        mount,
        plugin: plugin_id.value().to_string(),
        instance,
        region: region.id,
        window,
        host: anchors,
        state,
        revision,
        children,
        catcher,
    });
    let blocked = create_memo(move || state.with(|view| view.blocks_owner));
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
    let keyboard = create_memo(move || {
        state.with(|view| match view.ime.as_ref().map(|area| area.keyboard) {
            Some(beui::Keyboard::Held) => beui::SoftKeyboard::Held,
            Some(beui::Keyboard::Shown { asks }) => beui::SoftKeyboard::Shown { asks },
            None => beui::SoftKeyboard::Shown { asks: 0 },
        })
    });
    let ime_text = create_memo(move || {
        state.with(|view| view.ime.as_ref().and_then(|area| area.text.clone()))
    });
    let loading = create_memo(move || region.is_main() && state.with(|view| view.loading));
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
    let handing = StoredCell::new(handoff::Handoff::default());
    let forward = move |input: ForwardedInput| {
        let mut handed = handing.get();
        let input = handed.route(input);
        handing.set(handed);
        let origin = place.map_or(beui::Vec2::ZERO, |place| {
            place.display.get_untracked().min.to_vec2()
        });
        if hosts_windows || windowed {
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
                plugin_id.with(|plugin| super::desktop::aim(plugin, own, rect, over, pos))
            };
            let target = |node: beui::NodeId| {
                super::desktop::catcher(node).map(|held| (held.instance, held.region))
            };
            block_app_plugin_host::forward_view(
                &plugin_id.value(),
                instance,
                region.id,
                &input,
                &aim,
                &target,
            );
        }
    };
    let forward = Callback::new(forward);
    if let Some(block) = block.filter(|_| region.is_main()) {
        let used = create_memo(move || state.with(|view| view.used));
        create_effect(move || {
            revision.get();
            rect.get();
            let parent = enclosing
                .and_then(|Enclosing(parent)| parent.get())
                .and_then(|parent| parent.try_get());
            if let Some(node) = catcher.try_get() {
                handoff::register(
                    node,
                    handoff::Receiver {
                        block,
                        parent,
                        rect,
                        used,
                        forward,
                    },
                );
            }
        });
        create_effect(move || {
            used.get();
            if let Some(node) = catcher.try_get() {
                untrack(|| handoff::laid_out(node, now()));
            }
        });
        on_cleanup(move || {
            if let Some(node) = catcher.try_get() {
                handoff::forget(node);
            }
        });
    }
    let promoted = create_memo(move || {
        state.with(|view| {
            view.children
                .iter()
                .filter(|child| child.mode == block_plugin_api::ChildMode::Active)
                .filter_map(|child| {
                    let lands_in = (child.frame_owner && !child.own_frame).then_some(view.rect);
                    Some((child.block_id()?, (child.rect, lands_in)))
                })
                .collect::<Vec<_>>()
        })
    });
    let was_promoted = store(RefCell::new(Vec::<Uuid>::new()));
    let hand_to = move |to: NodeId, (within, lands_in): (Rect, Option<Rect>)| {
        let mut handed = handing.get();
        let cancel = handed.hand_to(to, within, lands_in, now());
        handing.set(handed);
        if let Some(cancel) = cancel {
            block_app_plugin_host::forward_view(
                &plugin_id.value(),
                instance,
                region.id,
                &cancel,
                &|_, _| beui::Aim::Own,
                &|_| None,
            );
        }
    };
    create_effect(move || {
        let promoted = promoted.get();
        untrack(|| {
            let blocks: Vec<Uuid> = promoted.iter().map(|(block, _)| *block).collect();
            let before = was_promoted.value().replace(blocks.clone());
            for gone in before.iter().filter(|block| !blocks.contains(block)) {
                handoff::stop_awaiting(*gone);
            }
            let Some(own) = catcher.try_get() else {
                return;
            };
            for (block, within) in promoted.iter().filter(|(block, _)| !before.contains(block)) {
                let within = *within;
                match handoff::receiver_of(own, *block) {
                    Some(to) => hand_to(to, within),
                    None => handoff::await_receiver(
                        *block,
                        Callback::new(move |to: NodeId| hand_to(to, within)),
                    ),
                }
            }
        });
    });
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
                    block_app_plugin_host::forward_view(
                        &plugin_id.value(),
                        instance,
                        region.id,
                        &cancel,
                        &|_, _| beui::Aim::Own,
                        &|_| None,
                    );
                }
            });
        });
    }
    let back = move |gesture: beui::BackGesture| {
        block_app_plugin_host::back_view(&plugin_id.value(), instance, region.id, gesture);
    };
    let occluded = occlusion();
    let beyond = move |local: Pos2| {
        let Some(placed) = frame.get_untracked().and_then(|frame| frame.content) else {
            return false;
        };
        let size = state.with_untracked(|view| view.rect.size());
        region.is_main()
            && !placed.contains(local)
            && block_app_plugin_host::frame_rects(&plugin_id.value(), instance, region.id)
                .is_some_and(|frame| {
                    frame
                        .content(Rect::from_min_size(Pos2::ZERO, size))
                        .contains(local)
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
        block_app_plugin_host::drag_over_view(&plugin_id.value(), instance, region.id, over, false);
    };
    let dropped = move |(drag, point): (BlockDrag, DragPoint)| {
        if taken(point) {
            block_app_plugin_host::drag_over_view(
                &plugin_id.value(),
                instance,
                region.id,
                Some((drag, point.pos)),
                true,
            );
        }
    };
    let pressed_outside = move |pos: Pos2| {
        if state.with_untracked(|view| view.anchored.iter().any(|layer| layer.takes(pos))) {
            return;
        }
        block_app_plugin_host::pressed_outside_view(&plugin_id.value(), instance, region.id);
    };
    let hit_area = create_memo(move || {
        if passive.get() {
            return beui::Area::Nowhere;
        }
        revision.get();
        let content = frame.get().and_then(|frame| frame.content);
        let beyond = match (region.is_main(), content) {
            (true, Some(placed)) => {
                block_app_plugin_host::frame_rects(&plugin_id.value(), instance, region.id)
                    .map_or_else(beui::HitRegion::new, |frame| {
                        let size = state.with(|view| view.rect.size());
                        frame.floating.iter().fold(
                            beui::HitRegion::rect(
                                frame.content(Rect::from_min_size(Pos2::ZERO, size)),
                            )
                            .without(placed),
                            |beyond, floating| beyond.without(*floating),
                        )
                    })
            }
            _ => beui::HitRegion::new(),
        };
        let occluders = occluded.map(|occluded| occluded.get()).unwrap_or_default();
        state.with(|view| {
            let origin = view.rect.min.to_vec2();
            let mut takes = beui::HitRegion::rect(Rect::EVERYTHING);
            for (hole, covers) in &view.holes {
                let open = covers
                    .iter()
                    .fold(beui::HitRegion::rect(*hole), |open, cover| {
                        open.without(*cover)
                    });
                for piece in open.rects() {
                    takes = takes.without(piece.translate(-origin));
                }
            }
            for piece in beyond.rects() {
                takes = takes.without(*piece);
            }
            for occluder in &occluders {
                takes = takes.without(occluder.translate(-origin));
            }
            beui::Area::Within(takes)
        })
    });
    let claims = create_memo(move || {
        if passive.get() {
            return Vec::new();
        }
        state.with(|view| {
            let origin = view.rect.min.to_vec2();
            view.claims
                .iter()
                .filter(|(held, _)| held.any())
                .map(|(held, rect)| {
                    beui::Claim::holding(*held, beui::HitRegion::rect(rect.translate(-origin)))
                })
                .collect::<Vec<_>>()
        })
    });
    let pen_surface = create_memo(move || {
        if passive.get() {
            return beui::Area::Nowhere;
        }
        state.with(|view| {
            let origin = view.rect.min.to_vec2();
            let drawn = view
                .drawing
                .iter()
                .fold(beui::HitRegion::new(), |drawn, rect| {
                    drawn.with(rect.translate(-origin))
                });
            beui::Area::Within(view.floating_rects.iter().fold(drawn, |drawn, floating| {
                drawn.without(floating.translate(-origin))
            }))
        })
    });
    let windows = windowing.map(|editor| {
        let overlaid = (!shows_desktop).then(|| {
            let editor = editor.clone();
            view! {
                <OverlaidWindows editor region nodes={children} />
            }
        });
        let panels = (!nests_windows).then(|| {
            let editor = editor.clone();
            view! {
                <PanelWindows editor region rect />
            }
        });
        view! {
            <Layers>{panels}{overlaid}</Layers>
        }
    });
    let built = view! {
        <DropTarget on_over={over} on_drop={dropped}>|_: DropHandle|
            <Layers>
                <ViewChildren state={state} rect={rect} nodes={children} below=true />
                <Interactive
                    @node_ref={&catcher}
                    focusable=true
                    press_focus={create_memo(move || !presented.get())}
                    cursor={cursor}
                    ime={ime}
                    keyboard={keyboard}
                    ime_cursor={ime_cursor}
                    ime_text={ime_text}
                    pastes={pastes}
                    clipboard_keys={clipboard_keys}
                    every_sample={every_sample}
                    on_forward={forward}
                    hit_area={hit_area}
                    claims={claims}
                    drawing={pen_surface}
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
                <Notices loading failure id={crash_id.clone()} />
                <Show condition={shows_desktop}>||
                    <DesktopWindows editor={desktop_editor.with(Clone::clone)} state rect />
                </Show>
                <ViewChildren state rect nodes={children} below=false />
                <Scrims state />
                <Drawing draw={floating} />
                <Show condition={blocked}>||
                    <Interactive press_focus=false on_forward={|_: ForwardedInput| {}} />
                </Show>
                {windows}
            </Layers>
        </DropTarget>
    };
    on_back(handles_back, back);
    built
}

#[component]
fn Scrims(state: Memo<PlacedView>) -> NodeId {
    let count = create_memo(move || state.with(|view| (0..view.scrims.len()).collect::<Vec<_>>()));
    view! {
        <Canvas>
            <ForEach keys={count}>|index: usize| {
                let scrim = create_memo(move || {
                    state.with(|view| {
                        view.scrims.get(index).map(|scrim| beui::layer::Scrim {
                            rect: scrim.rect.translate(-view.rect.min.to_vec2()),
                            ..*scrim
                        })
                    })
                });
                let rect = create_memo(move || scrim.get().map_or(Rect::ZERO, |scrim| scrim.rect));
                let fill = create_memo(move || {
                    ColorRole::Custom(beui::color::Color::literal(
                        scrim.get().map_or(beui::color::Color32::TRANSPARENT, |scrim| scrim.color),
                    ))
                });
                let radius =
                    create_memo(move || Radius::Custom(scrim.get().map_or(0.0, |scrim| scrim.radius)));
                view! {
                    <CanvasItem
                        x={create_memo(move || rect.get().min.x)}
                        y={create_memo(move || rect.get().min.y)}
                        width={create_memo(move || rect.get().width())}
                        height={create_memo(move || rect.get().height())}
                    >
                        <Frame fill={fill} radius={radius} />
                    </CanvasItem>
                }
            }</ForEach>
        </Canvas>
    }
}

#[component]
fn DesktopWindows(editor: ViewEditor, state: Memo<PlacedView>, rect: Memo<Rect>) -> NodeId {
    let place = super::desktop::place().unwrap_or_else(|| {
        let display = create_memo(move || Rect::from_min_size(Pos2::ZERO, rect.get().size()));
        super::desktop::DesktopPlace {
            display,
            displays: create_memo(move || vec![display.get()]),
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
            <ForEach keys={keys}>|window: ViewId| {
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
                let opens_at = create_memo(move || local.get().translate(rect.get().min.to_vec2()));
                let region = View::window(window);
                let child_view = super::editors::child_view(
                    block.unwrap_or_else(|| super::editors::hosted_parent(editor.instance)),
                    region,
                );
                view! {
                    <CanvasItem x={x} y={y} width={width} height={height}>
                        <PluginView
                            editor
                            region
                            child_view
                            opens_at={Some(opens_at)}
                            on_focus={Callback::new(|()| {})}
                        />
                    </CanvasItem>
                }
            }</ForEach>
        </Canvas>
    }
}

thread_local! {
    static NEXT_ANCHOR_HOST: Cell<u64> = const { Cell::new(0) };
}

#[component]
fn OverlaidWindows(editor: ViewEditor, region: View, nodes: ChildNodes) -> NodeId {
    let block = editor.role.block().map(|block| block.id);
    let keys = super::desktop::overlaid(editor.plugin_id().to_owned(), editor.instance, region);
    let viewport = use_viewport_size();
    let editor = store(editor);
    view! {
        <Layers>
            <ForEach keys={keys}>|window: ViewId| {
                let editor = editor.with(Clone::clone);
                let key = super::desktop::FloatedKey {
                    plugin: editor.plugin_id().to_owned(),
                    instance: editor.instance,
                    window,
                };
                let shown = super::desktop::carried(key, viewport);
                let anchor = create_memo(move || OverlayAnchor::Point(shown.get().min));
                let width = create_memo(move || Size::Custom(shown.get().width()));
                let height = create_memo(move || Size::Custom(shown.get().height()));
                let whole_width = create_memo(move || Size::Custom(viewport.get().x));
                let whole_height = create_memo(move || Size::Custom(viewport.get().y));
                let region = View::window(window);
                let child_view = super::editors::child_view(
                    block.unwrap_or_else(|| super::editors::hosted_parent(editor.instance)),
                    region,
                );
                let held = store(editor);
                let host = Some(super::anchored::AnchorHost(NEXT_ANCHOR_HOST.with(|next| {
                    next.set(next.get() + 1);
                    next.get()
                })));
                let window = {
                    provide_context(super::anchored::HostOfAnchors(create_memo(move || host)));
                    view! {
                        <Overlay
                            anchor
                            placement=Placement::At
                            policy=LayerPolicy::Pass
                            traps_focus=false
                            open=true
                            dismissable=false
                            on_dismiss={|_: Dismiss| {}}
                        >
                            <Frame width height>
                                <PluginView
                                    editor={held.with(Clone::clone)}
                                    region
                                    nests_windows=true
                                    opens_at={Some(shown)}
                                    shares={Some(nodes)}
                                    child_view
                                    on_focus={Callback::new(|()| {})}
                                />
                            </Frame>
                        </Overlay>
                    }
                };
                view! {
                    <Layers>
                        {window}
                        <Overlay
                            anchor={OverlayAnchor::Point(Pos2::ZERO)}
                            placement=Placement::At
                            policy=LayerPolicy::Pass
                            traps_focus=false
                            open=true
                            dismissable=false
                            on_dismiss={|_: Dismiss| {}}
                        >
                            <Frame width=whole_width height=whole_height>
                                <AnchoredViews host />
                            </Frame>
                        </Overlay>
                    </Layers>
                }
            }</ForEach>
        </Layers>
    }
}

#[component]
fn PanelWindows(editor: ViewEditor, region: View, rect: Memo<Rect>) -> NodeId {
    let instance = editor.instance;
    let plugin = editor.plugin_id().to_owned();
    let block = editor.role.block().map(|block| block.id);
    let keys = super::desktop::panels(plugin.clone(), instance, region);
    let plugin = store(plugin);
    let editor = store(editor);
    let hosted: Stored<RefCell<HashMap<ViewId, WindowId>>> = store(RefCell::default());
    let within = use_window().map_or(WindowId::MAIN, |handle| handle.id);
    let placed = move |placement: block_plugin_api::WindowPlacement| match placement {
        block_plugin_api::WindowPlacement::AtPointer => WindowPlacement::AtPointer,
        block_plugin_api::WindowPlacement::Host(placement) => {
            WindowPlacement::Host(beui::HostPlacement(placement))
        }
        block_plugin_api::WindowPlacement::Over(over) => {
            let opener = over.view.view;
            match hosted.with(|hosted| hosted.borrow().get(&opener).copied()) {
                Some(window) => WindowPlacement::Over {
                    window,
                    at: over.at,
                },
                None if opener == region.id && over.view.instance == instance => {
                    WindowPlacement::Over {
                        window: within,
                        at: over.at + rect.get_untracked().min.to_vec2(),
                    }
                }
                None => WindowPlacement::AtPointer,
            }
        }
    };
    view! {
        <Layers>
            <ForEach keys={keys}>|window: ViewId| {
                let id = WindowId::next();
                hosted.with(|hosted| hosted.borrow_mut().insert(window, id));
                on_cleanup(move || {
                    hosted.with(|hosted| hosted.borrow_mut().remove(&window));
                });
                let key = super::desktop::FloatedKey {
                    plugin: plugin.value().to_string(),
                    instance,
                    window,
                };
                let panel = super::desktop::panel_of(key.clone());
                let size = create_memo(move || panel.with(|panel| panel.as_ref().map_or(Vec2::ZERO, |(size, _)| *size)));
                let opened = create_memo(move || {
                    panel.with(|panel| panel.as_ref().map(|(_, panel)| panel.opened.clone()))
                });
                let placement = create_memo(move || match opened.get() {
                    Some(opened) => placed(opened),
                    None => WindowPlacement::AtPointer,
                });
                let reported = key.clone();
                let report = move |report: WindowReport| match report {
                    WindowReport::Placed { placement, .. } => {
                        super::desktop::panel_placed(&reported, placement.0);
                    }
                    WindowReport::Closed(_) => super::desktop::dismiss(&reported),
                    _ => {}
                };
                let region = View::window(window);
                let child_view = super::editors::child_view(
                    block.unwrap_or_else(|| super::editors::hosted_parent(instance)),
                    region,
                );
                view! {
                    <Window id={Some(id)} size placement on_report={report}>|handle: WindowHandle| {
                        follow_panel_asks(panel, handle);
                        let hosted = create_memo(move || handle.is_window());
                        view! {
                            <Layers>
                                <PluginView
                                    editor={editor.with(ViewEditor::clone)}
                                    region
                                    child_view
                                    on_focus={Callback::new(|()| {})}
                                />
                                <Show condition={hosted}>||
                                    <AnchoredViews />
                                </Show>
                            </Layers>
                        }
                    }</Window>
                }
            }</ForEach>
        </Layers>
    }
}

fn follow_panel_asks(panel: Memo<Option<(Vec2, super::desktop::Panel)>>, handle: WindowHandle) {
    let asked = create_memo(move || {
        panel.with(|panel| {
            panel
                .as_ref()
                .map_or((0, 0, beui::WindowEdge::BottomRight), |(_, panel)| {
                    (panel.moves, panel.resizes.0, edge_of(panel.resizes.1))
                })
        })
    });
    let seen = StoredCell::new(asked.get_untracked());
    create_effect(move || {
        let (moves, resizes, edge) = asked.get();
        let (moved, resized, _) = seen.get();
        seen.set((moves, resizes, edge));
        untrack(|| {
            if moves != moved {
                handle.begin_move();
            }
            if resizes != resized {
                handle.begin_resize(edge);
            }
        });
    });
}

fn edge_of(edge: block_plugin_api::WindowEdge) -> beui::WindowEdge {
    use block_plugin_api::WindowEdge as Edge;
    match edge {
        Edge::Top => beui::WindowEdge::Top,
        Edge::Bottom => beui::WindowEdge::Bottom,
        Edge::Left => beui::WindowEdge::Left,
        Edge::Right => beui::WindowEdge::Right,
        Edge::TopLeft => beui::WindowEdge::TopLeft,
        Edge::TopRight => beui::WindowEdge::TopRight,
        Edge::BottomLeft => beui::WindowEdge::BottomLeft,
        Edge::BottomRight => beui::WindowEdge::BottomRight,
    }
}

fn carry(drag: BlockDrag) {
    let Some(pointer) = try_with_document(|document| document.last_pointer) else {
        return;
    };
    let board = beui::unstyled::drag_board();
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
    region: View,
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
            if !block_app_plugin_host::take_focus_request(&plugin_id.value(), instance, region.id) {
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

fn toggle_keyboard_when_asked(state: Memo<PlacedView>, primary: Memo<bool>) {
    let toggles = create_memo(move || state.with(|view| view.keyboard_toggles));
    let seen = StoredCell::new(None::<u32>);
    create_effect(move || {
        let Some(toggles) = toggles.get() else {
            return;
        };
        let before = seen.get();
        seen.set(Some(toggles));
        if before.is_some_and(|before| before != toggles) && primary.get_untracked() {
            untrack(toggle_keyboard);
        }
    });
}

fn take_keyboard(
    state: Memo<PlacedView>,
    given: Option<Memo<bool>>,
    catcher: &NodeRef,
    primary: Memo<bool>,
) {
    let wants = create_memo(move || {
        (state.with(|view| view.wants_keyboard) || given.is_some_and(|given| given.get()))
            && primary.get()
    });
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
            block_app_base::host::push_command(block_app_base::host::HostCommand::RestartPlugin(
                plugin,
            ));
        }
    };
    let loading_item = create_memo(|| HostItem::Notice {
        text: "Loading plugin…".to_owned(),
        busy: true,
        icon: None,
    });
    view! {
        <Frame align=Align::Center>
            <List spacing=Space::None>
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
fn ViewChildren(
    state: Memo<PlacedView>,
    rect: Memo<Rect>,
    nodes: ChildNodes,
    below: bool,
) -> NodeId {
    let keys = create_memo(move || {
        state.with(|view| {
            view.children
                .iter()
                .filter(|child| child.anchored.is_none())
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
                let built = nodes.node(key);
                view! {
                    <CanvasItem x={x} y={y} width={width} height={height}>
                        <Portal node={Some(built)} />
                    </CanvasItem>
                }
            }</ForEach>
        </Canvas>
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Joined {
    pub(crate) mount: u64,
    pub(crate) state: Memo<PlacedView>,
    pub(crate) rect: Memo<Rect>,
    pub(crate) child_view: ChildView,
    pub(crate) anchors: Memo<Option<super::anchored::AnchorHost>>,
    pub(crate) encloses: Memo<Option<NodeRef>>,
}

impl Joined {
    fn lists(&self, key: ChildId) -> bool {
        self.state.is_alive()
            && self
                .state
                .with(|view| view.children.iter().any(|child| child.child == key))
    }
}

struct BuiltChild {
    node: NodeId,
    scope: Option<reactive::Scope>,
    holder: Signal<u64>,
}

struct BuiltChildren {
    region: Option<reactive::ScopeContext>,
    views: Signal<Vec<Joined>>,
    built: RefCell<HashMap<ChildId, BuiltChild>>,
}

#[derive(Clone, Copy)]
pub(crate) struct ChildNodes {
    shared: Stored<BuiltChildren>,
    mount: u64,
    child_view: ChildView,
}

impl ChildNodes {
    pub(crate) fn joining(shares: Option<ChildNodes>, joined: Joined) -> Self {
        let mount = joined.mount;
        let child_view = joined.child_view;
        let shared = match shares.filter(|shares| shares.shared.is_alive()) {
            Some(shares) => {
                let views = shares.shared.with(|held| held.views);
                views.update(|views| views.push(joined));
                on_cleanup(move || {
                    if views.is_alive() {
                        views.update(|views| views.retain(|view| view.mount != mount));
                    }
                });
                shares.shared
            }
            None => Self::root(joined),
        };
        Self {
            shared,
            mount,
            child_view,
        }
    }

    fn root(joined: Joined) -> Stored<BuiltChildren> {
        let views = create_signal(vec![joined]);
        let shared = store(BuiltChildren {
            region: reactive::current_scope(),
            views,
            built: RefCell::new(HashMap::new()),
        });
        create_effect(move || {
            let shown: Vec<ChildId> = views.with(|views| {
                views
                    .iter()
                    .filter(|view| view.state.is_alive())
                    .flat_map(|view| {
                        view.state.with(|view| {
                            view.children
                                .iter()
                                .map(|child| child.child)
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect()
            });
            untrack(|| keep(shared, &shown));
        });
        on_cleanup(move || keep(shared, &[]));
        shared
    }

    pub(crate) fn node(self, key: ChildId) -> NodeId {
        let held = self.shared.with(|held| {
            held.built
                .borrow()
                .get(&key)
                .map(|built| (built.node, built.holder))
        });
        if let Some((node, holder)) = held {
            if holder.is_alive() && holder.get_untracked() != self.mount {
                holder.set(self.mount);
            }
            return node;
        }
        let (views, region) = self.shared.with(|held| (held.views, held.region.clone()));
        let child_view = self.child_view;
        let mount = self.mount;
        let build = move || {
            let holder = create_signal(mount);
            let holding = create_memo(move || {
                let mount = holder.get();
                views.with(|views| {
                    views
                        .iter()
                        .find(|view| view.mount == mount && view.lists(key))
                        .or_else(|| views.iter().find(|view| view.lists(key)))
                        .map(|view| view.mount)
                })
            });
            let following = move || holding.get().and_then(|mount| joined(views, mount));
            let child = create_memo(move || {
                let view = following()?;
                view.state.with(|view| {
                    view.children
                        .iter()
                        .find(|child| child.child == key)
                        .cloned()
                })
            });
            let rect = create_memo(move || following().map_or(Rect::ZERO, |view| view.rect.get()));
            let anchors = create_memo(move || following().and_then(|view| view.anchors.get()));
            let encloses = create_memo(move || following().and_then(|view| view.encloses.get()));
            provide_context(super::anchored::HostOfAnchors(anchors));
            provide_context(Enclosing(encloses));
            let node = child_view.with(|view| view(key, child, rect));
            (node, holder)
        };
        let scope = region.as_ref().and_then(super::anchored::closed_with);
        let (node, holder) = match &scope {
            Some(scope) => scope.run(build),
            None => build(),
        };
        self.shared.with(|held| {
            held.built.borrow_mut().insert(
                key,
                BuiltChild {
                    node,
                    scope,
                    holder,
                },
            )
        });
        node
    }
}

fn joined(views: Signal<Vec<Joined>>, mount: u64) -> Option<Joined> {
    if !views.is_alive() {
        return None;
    }
    views.with(|views| views.iter().find(|view| view.mount == mount).copied())
}

fn keep(shared: Stored<BuiltChildren>, shown: &[ChildId]) {
    if !shared.is_alive() {
        return;
    }
    let gone: Vec<Option<reactive::Scope>> = shared.with(|held| {
        let mut built = held.built.borrow_mut();
        let stale: Vec<ChildId> = built
            .keys()
            .filter(|key| !shown.contains(key))
            .copied()
            .collect();
        stale
            .into_iter()
            .filter_map(|key| built.remove(&key).map(|built| built.scope))
            .collect()
    });
    for scope in gone.into_iter().flatten() {
        scope.dispose();
    }
}

#[cfg(test)]
mod tests;
