use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use beui::native::{Region, draw_gpu, try_with_document};
use beui::reactive::*;
use beui::styled::*;
use beui::{Forwarded, ForwardedInput, NodeId, PointerPhase, Pos2, Rect, WindowId};
use block_app_plugin_host::{AnchoredBlits, HostAnchored, HostChild, PlacedView};
use block_plugin_api::{ChildId, EditorInstanceId, ViewId};

use super::plugin_view::ChildNodes;

#[derive(Clone, Copy)]
pub(crate) struct AnchorBounds(pub(crate) Memo<Rect>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnchorHost(pub(crate) u64);

pub(crate) fn anchor_host() -> Option<AnchorHost> {
    use_context::<AnchorHost>()
}

#[derive(Clone)]
pub(crate) struct AnchorOwner {
    pub(crate) mount: u64,
    pub(crate) plugin: String,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: ViewId,
    pub(crate) window: Memo<WindowId>,
    pub(crate) host: Option<AnchorHost>,
    pub(crate) state: Memo<PlacedView>,
    pub(crate) revision: ReadSignal<u64>,
    pub(crate) children: ChildNodes,
    pub(crate) catcher: NodeRef,
}

thread_local! {
    static OWNERS: RefCell<Vec<AnchorOwner>> = const { RefCell::new(Vec::new()) };
    static CHANGED: RefCell<Option<Signal<u64>>> = const { RefCell::new(None) };
}

pub(crate) fn install() {
    let changed = create_signal(0u64);
    CHANGED.with(|held| *held.borrow_mut() = Some(changed));
}

fn changed() {
    if let Some(changed) = CHANGED.with(|held| *held.borrow()) {
        changed.update(|revision| *revision += 1);
    }
}

fn watch_owners() {
    if let Some(changed) = CHANGED.with(|held| *held.borrow()) {
        changed.get();
    }
}

pub(crate) fn register(owner: AnchorOwner) {
    let mount = owner.mount;
    OWNERS.with(|owners| owners.borrow_mut().push(owner));
    changed();
    on_cleanup(move || {
        OWNERS.with(|owners| owners.borrow_mut().retain(|owner| owner.mount != mount));
        changed();
    });
}

fn owner(mount: u64) -> Option<AnchorOwner> {
    OWNERS.with(|owners| {
        owners
            .borrow()
            .iter()
            .find(|owner| owner.mount == mount)
            .cloned()
    })
}

fn owners_in(window: WindowId, host: Option<AnchorHost>) -> Vec<AnchorOwner> {
    OWNERS.with(|owners| {
        owners
            .borrow()
            .iter()
            .filter(|owner| owner.state.is_alive() && owner.window.is_alive())
            .filter(|owner| owner.window.get() == window && owner.host == host)
            .cloned()
            .collect()
    })
}

pub(crate) fn host_window() -> Memo<WindowId> {
    let real = use_window().map(|window| window.real());
    create_memo(move || {
        real.filter(Memo::is_alive)
            .and_then(|real| real.get())
            .unwrap_or(WindowId::MAIN)
    })
}

pub(crate) fn bounds_within(parent: Rect, own: Rect, confined: bool) -> Rect {
    match confined {
        true => own.intersect(parent),
        false => parent,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Shown {
    mount: u64,
    anchored: HostAnchored,
}

fn shown_anchored(owners: &[AnchorOwner]) -> Vec<Shown> {
    let mut shown: Vec<Shown> = owners
        .iter()
        .flat_map(|owner| {
            owner.state.with(|view| {
                view.anchored
                    .iter()
                    .map(|layer| Shown {
                        mount: owner.mount,
                        anchored: layer.clone(),
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    shown.sort_by_key(|shown| shown.anchored.opened);
    shown
}

fn topmost(shown: &[Shown], pos: Pos2) -> Option<Target> {
    shown
        .iter()
        .rev()
        .find(|shown| shown.anchored.takes(pos))
        .map(|shown| (shown.mount, shown.anchored.id))
}

fn taken(shown: &[Shown], holes: &[(u64, ViewId, Rect)]) -> beui::HitRegion {
    shown.iter().fold(beui::HitRegion::new(), |takes, shown| {
        let takes = shown
            .anchored
            .floating
            .iter()
            .fold(takes, |takes, rect| takes.with(*rect));
        holes
            .iter()
            .filter(|(mount, layer, _)| *mount == shown.mount && *layer == shown.anchored.id)
            .fold(takes, |takes, (_, _, hole)| takes.without(*hole))
    })
}

type Target = (u64, ViewId);

#[derive(Default)]
struct Pointing {
    held: Cell<Option<Target>>,
    over: Cell<Option<Target>>,
}

fn route(
    input: &ForwardedInput,
    shown: &[Shown],
    pointing: &Pointing,
) -> Vec<(Target, Vec<Forwarded>, bool)> {
    let Pointing { held, over } = pointing;
    let mut runs: Vec<(Target, Vec<Forwarded>, bool)> = Vec::new();
    for event in input.events.iter().cloned() {
        let (target, pressed) = match &event {
            Forwarded::Pointer { event: pointer, .. } => {
                let (target, pressed) = match pointer.phase {
                    PointerPhase::Down(_) => {
                        let target = topmost(shown, pointer.pos);
                        held.set(target);
                        (target, true)
                    }
                    PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => {
                        (held.take().or_else(|| topmost(shown, pointer.pos)), false)
                    }
                    _ => (held.get().or_else(|| topmost(shown, pointer.pos)), false),
                };
                let gone = pointer.phase == PointerPhase::Gone;
                let left = over.replace(target.filter(|_| !gone));
                if let Some(left) = left.filter(|left| gone || Some(*left) != target) {
                    push(&mut runs, left, Forwarded::Left, false);
                }
                (target, pressed)
            }
            Forwarded::Left => (
                held.get()
                    .or(over.take())
                    .or_else(|| input.pointer.and_then(|pos| topmost(shown, pos))),
                false,
            ),
            Forwarded::Focus(_) | Forwarded::Input(_) => (None, false),
        };
        let Some(target) = target else {
            continue;
        };
        push(&mut runs, target, event, pressed);
    }
    runs
}

fn push(
    runs: &mut Vec<(Target, Vec<Forwarded>, bool)>,
    target: Target,
    event: Forwarded,
    pressed: bool,
) {
    match runs.last_mut() {
        Some((last, events, presses)) if *last == target => {
            events.push(event);
            *presses |= pressed;
        }
        _ => runs.push((target, vec![event], pressed)),
    }
}

fn forward(input: ForwardedInput, shown: &[Shown], pointing: &Pointing) {
    for ((mount, layer), events, pressed) in route(&input, shown, pointing) {
        let Some(owner) = owner(mount) else {
            continue;
        };
        if pressed {
            focus_owner(&owner);
        }
        block_app_plugin_host::forward_anchored(
            &owner.plugin,
            owner.instance,
            owner.region,
            layer,
            &ForwardedInput {
                events,
                ..input.clone()
            },
        );
    }
}

fn focus_owner(owner: &AnchorOwner) {
    let Some(node) = owner.catcher.try_get() else {
        return;
    };
    try_with_document(|document| {
        if document.focused_node() != Some(node) {
            document.focus_focusable(node);
        }
    });
}

fn dismiss_outside(window: WindowId, host: Option<AnchorHost>, pos: Pos2) {
    for owner in owners_in(window, host) {
        let layers = owner.state.with_untracked(|view| view.anchored.clone());
        let dismissed = block_app_plugin_host::dismissed_by_press(&layers, pos);
        if !dismissed.is_empty() {
            block_app_plugin_host::dismiss_anchored_views(
                &owner.plugin,
                owner.instance,
                owner.region,
                dismissed,
            );
        }
    }
}

#[component]
pub(crate) fn AnchoredViews(#[prop(default = None)] host: Option<AnchorHost>) -> NodeId {
    let window = host_window();
    let owners = create_memo(move || {
        watch_owners();
        owners_in(window.get(), host)
            .into_iter()
            .map(|owner| owner.mount)
            .collect::<Vec<u64>>()
    });
    let shown = create_memo(move || {
        let owners: Vec<AnchorOwner> = owners.get().into_iter().filter_map(owner).collect();
        shown_anchored(&owners)
    });
    let keys = create_memo(move || {
        shown.with(|shown| {
            shown
                .iter()
                .map(|shown| (shown.mount, shown.anchored.id))
                .collect::<Vec<_>>()
        })
    });
    let pointing: Rc<Pointing> = Rc::new(Pointing::default());
    let on_forward = move |input: ForwardedInput| {
        shown.with_untracked(|shown| forward(input, shown, &pointing));
    };
    let hit_area = create_memo(move || {
        let holes: Vec<(u64, ViewId, Rect)> = owners
            .get()
            .into_iter()
            .filter_map(owner)
            .flat_map(|owner| {
                owner.state.with(|view| {
                    view.children
                        .iter()
                        .filter(|child| child.is_active())
                        .filter_map(|child| {
                            Some((
                                owner.mount,
                                child.anchored?,
                                child.rect.intersect(child.clip),
                            ))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        shown.with(|shown| {
            if shown.is_empty() {
                return beui::Area::Nowhere;
            }
            beui::Area::Within(taken(shown, &holes))
        })
    });
    let watched = create_memo(move || shown.with(|shown| !shown.is_empty()));
    view! {
        <Layers>
            <ForEach keys={keys}>|(mount, id): (u64, ViewId)|
                <AnchoredView mount id shown />
            </ForEach>
            <Interactive hit_area={hit_area} on_forward={on_forward} press_focus=false />
            <Show condition={watched}>||
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=0.0 height=0.0>
                        <Interactive
                            hit_area={beui::Area::Nowhere}
                            on_press_outside={move |pos: Pos2| dismiss_outside(window.get_untracked(), host, pos)}
                        />
                    </CanvasItem>
                </Canvas>
            </Show>
        </Layers>
    }
}

#[component]
fn AnchoredView(mount: u64, id: ViewId, shown: Memo<Vec<Shown>>) -> NodeId {
    let layer = create_memo(move || {
        shown.with(|shown| {
            shown
                .iter()
                .find(|shown| shown.mount == mount && shown.anchored.id == id)
                .map(|shown| shown.anchored.clone())
        })
    });
    let scrim_draw = Prop::dynamic(move || -> Draw {
        let scrims = layer.with(|layer| {
            layer
                .as_ref()
                .map(|layer| layer.scrims.clone())
                .unwrap_or_default()
        });
        Rc::new(move |painter: &Painter, rect: Rect| {
            for scrim in &scrims {
                painter.rect_filled(
                    scrim.rect.translate(rect.min.to_vec2()),
                    scrim.radius,
                    beui::color::Color::literal(scrim.color),
                );
            }
        })
    });
    let children = create_memo(move || {
        let Some(owner) = owner(mount).filter(|owner| owner.state.is_alive()) else {
            return Vec::new();
        };
        owner.state.with(|view| {
            view.children
                .iter()
                .filter(|child| child.anchored == Some(id))
                .map(|child| (child.child, child.is_below()))
                .collect::<Vec<_>>()
        })
    });
    let below = create_memo(move || {
        children.with(|children| {
            children
                .iter()
                .filter(|(_, below)| *below)
                .map(|(child, _)| (mount, *child))
                .collect::<Vec<_>>()
        })
    });
    let above = create_memo(move || {
        children.with(|children| {
            children
                .iter()
                .filter(|(_, below)| !*below)
                .map(|(child, _)| (mount, *child))
                .collect::<Vec<_>>()
        })
    });
    let blits: AnchoredBlits = Rc::new(RefCell::new(Vec::new()));
    let held: Rc<RefCell<Option<beui::native::Drawing>>> = Rc::new(RefCell::new(None));
    let pixels = create_memo(move || {
        let owner = owner(mount)?;
        owner.revision.get();
        let drawn = layer.get().map_or_else(Vec::new, |layer| {
            block_app_plugin_host::anchored_blits(&owner.plugin, &layer)
        });
        let damage = blits
            .borrow()
            .iter()
            .chain(drawn.iter())
            .fold(Region::default(), |damage, blit| {
                damage.union(Region::from(blit.rect().expand(1.0)))
            });
        *blits.borrow_mut() = drawn;
        Some(
            held.borrow_mut()
                .get_or_insert_with(|| block_app_plugin_host::anchored_drawing(Rc::clone(&blits)))
                .redrawn(damage),
        )
    });
    view! {
        <Layers>
            <Drawing draw={scrim_draw} />
            <AnchoredChildren keys={below} />
            <Drawing draw={Prop::dynamic(move || draw_gpu(pixels.get()))} />
            <AnchoredChildren keys={above} />
        </Layers>
    }
}

pub(crate) fn closed_with(region: &reactive::ScopeContext) -> Option<reactive::Scope> {
    region.is_alive().then(|| region.run(reactive::Scope::new))
}

#[component]
fn AnchoredChildren(keys: Memo<Vec<(u64, ChildId)>>) -> NodeId {
    view! {
        <Canvas>
            <ForEach keys={keys}>|key: (u64, ChildId)| {
                let (mount, id) = key;
                let held = owner(mount);
                let child = create_memo(move || {
                    let owner = owner(mount)?;
                    if !owner.state.is_alive() {
                        return None;
                    }
                    owner.state.with(|view| {
                        view.children.iter().find(|child| child.child == id).cloned()
                    })
                });
                let clip = create_memo(move || child.get().map_or(Rect::ZERO, |child: HostChild| child.clip));
                let built = held.map(|owner| owner.children.node(id));
                let x = create_memo(move || clip.get().min.x);
                let y = create_memo(move || clip.get().min.y);
                let width = create_memo(move || clip.get().width());
                let height = create_memo(move || clip.get().height());
                view! {
                    <CanvasItem x={x} y={y} width={width} height={height}>
                        <Portal node={built} />
                    </CanvasItem>
                }
            }</ForEach>
        </Canvas>
    }
}

#[cfg(test)]
mod tests;
