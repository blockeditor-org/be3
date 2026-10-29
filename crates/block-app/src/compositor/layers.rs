use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use beui::native::{Region, draw_gpu, try_with_document};
use beui::reactive::*;
use beui::styled::*;
use beui::{Forwarded, ForwardedInput, NodeId, PointerPhase, Pos2, Rect, WindowId};
use block_app_plugin_host::{HostChild, HostLayer, LayerBlits, RegionView};
use block_plugin_api::{ChildId, EditorInstanceId, EditorRegion, LayerId};

use super::region::ChildView;

#[derive(Clone, Copy)]
pub(crate) struct LayerBounds(pub(crate) Memo<Rect>);

#[derive(Clone, Copy)]
pub(crate) struct NoLayers;

#[derive(Clone)]
pub(crate) struct LayerOwner {
    pub(crate) mount: u64,
    pub(crate) plugin: String,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
    pub(crate) window: WindowId,
    pub(crate) state: Memo<RegionView>,
    pub(crate) revision: ReadSignal<u64>,
    pub(crate) child_view: ChildView,
    pub(crate) scope: Option<reactive::ScopeContext>,
    pub(crate) catcher: NodeRef,
}

thread_local! {
    static OWNERS: RefCell<Vec<LayerOwner>> = const { RefCell::new(Vec::new()) };
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

pub(crate) fn register(owner: LayerOwner) {
    let mount = owner.mount;
    OWNERS.with(|owners| owners.borrow_mut().push(owner));
    changed();
    on_cleanup(move || {
        OWNERS.with(|owners| owners.borrow_mut().retain(|owner| owner.mount != mount));
        changed();
    });
}

fn owner(mount: u64) -> Option<LayerOwner> {
    OWNERS.with(|owners| {
        owners
            .borrow()
            .iter()
            .find(|owner| owner.mount == mount)
            .cloned()
    })
}

fn owners_in(window: WindowId) -> Vec<LayerOwner> {
    OWNERS.with(|owners| {
        owners
            .borrow()
            .iter()
            .filter(|owner| owner.window == window && owner.state.is_alive())
            .cloned()
            .collect()
    })
}

pub(crate) fn host_window() -> WindowId {
    use_window()
        .and_then(WindowHandle::real_window)
        .unwrap_or(WindowId::MAIN)
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
    layer: HostLayer,
}

fn shown_layers(owners: &[LayerOwner]) -> Vec<Shown> {
    let mut shown: Vec<Shown> = owners
        .iter()
        .flat_map(|owner| {
            owner.state.with(|view| {
                view.layers
                    .iter()
                    .map(|layer| Shown {
                        mount: owner.mount,
                        layer: layer.clone(),
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    shown.sort_by_key(|shown| shown.layer.opened);
    shown
}

fn topmost(shown: &[Shown], pos: Pos2) -> Option<Target> {
    shown
        .iter()
        .rev()
        .find(|shown| shown.layer.takes(pos))
        .map(|shown| (shown.mount, shown.layer.id))
}

fn taken(shown: &[Shown], holes: &[(u64, LayerId, Rect)]) -> beui::HitRegion {
    shown.iter().fold(beui::HitRegion::new(), |takes, shown| {
        let takes = shown
            .layer
            .floating
            .iter()
            .fold(takes, |takes, rect| takes.with(*rect));
        holes
            .iter()
            .filter(|(mount, layer, _)| *mount == shown.mount && *layer == shown.layer.id)
            .fold(takes, |takes, (_, _, hole)| takes.without(*hole))
    })
}

type Target = (u64, LayerId);

fn route(
    input: &ForwardedInput,
    shown: &[Shown],
    held: &Cell<Option<Target>>,
) -> Vec<(Target, Vec<Forwarded>, bool)> {
    let mut runs: Vec<(Target, Vec<Forwarded>, bool)> = Vec::new();
    for event in input.events.iter().cloned() {
        let (target, pressed) = match &event {
            Forwarded::Pointer { event: pointer, .. } => match pointer.phase {
                PointerPhase::Down(_) => {
                    let target = topmost(shown, pointer.pos);
                    held.set(target);
                    (target, true)
                }
                PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => {
                    (held.take().or_else(|| topmost(shown, pointer.pos)), false)
                }
                _ => (held.get().or_else(|| topmost(shown, pointer.pos)), false),
            },
            Forwarded::Left => (
                held.get()
                    .or_else(|| input.pointer.and_then(|pos| topmost(shown, pos))),
                false,
            ),
            Forwarded::Focus(_) | Forwarded::Input(_) => (None, false),
        };
        let Some(target) = target else {
            continue;
        };
        match runs.last_mut() {
            Some((last, events, presses)) if *last == target => {
                events.push(event);
                *presses |= pressed;
            }
            _ => runs.push((target, vec![event], pressed)),
        }
    }
    runs
}

fn forward(input: ForwardedInput, shown: &[Shown], held: &Cell<Option<Target>>) {
    for ((mount, layer), events, pressed) in route(&input, shown, held) {
        let Some(owner) = owner(mount) else {
            continue;
        };
        if pressed {
            focus_owner(&owner);
        }
        block_app_plugin_host::forward_layer(
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

fn focus_owner(owner: &LayerOwner) {
    let Some(node) = owner.catcher.try_get() else {
        return;
    };
    try_with_document(|document| {
        if document.focused_node() != Some(node) {
            document.focus_focusable(node);
        }
    });
}

fn dismiss_outside(window: WindowId, pos: Pos2) {
    for owner in owners_in(window) {
        let layers = owner.state.with_untracked(|view| view.layers.clone());
        let dismissed = block_app_plugin_host::dismissed_by_press(&layers, pos);
        if !dismissed.is_empty() {
            block_app_plugin_host::dismiss_layers(
                &owner.plugin,
                owner.instance,
                owner.region,
                dismissed,
            );
        }
    }
}

#[component]
pub(crate) fn PluginLayers() -> NodeId {
    let window = host_window();
    let owners = create_memo(move || {
        watch_owners();
        owners_in(window)
            .into_iter()
            .map(|owner| owner.mount)
            .collect::<Vec<u64>>()
    });
    let shown = create_memo(move || {
        let owners: Vec<LayerOwner> = owners.get().into_iter().filter_map(owner).collect();
        shown_layers(&owners)
    });
    let keys = create_memo(move || {
        shown.with(|shown| {
            shown
                .iter()
                .map(|shown| (shown.mount, shown.layer.id))
                .collect::<Vec<_>>()
        })
    });
    let pressing: Rc<Cell<Option<Target>>> = Rc::new(Cell::new(None));
    let on_forward = move |input: ForwardedInput| {
        shown.with_untracked(|shown| forward(input, shown, &pressing));
    };
    let hit_area = create_memo(move || {
        let holes: Vec<(u64, LayerId, Rect)> = owners
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
                                child.in_layer?,
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
            <ForEach keys={keys}>|(mount, id): (u64, LayerId)|
                <PluginLayer mount id shown />
            </ForEach>
            <Interactive hit_area={hit_area} on_forward={on_forward} press_focus=false />
            <Show condition={watched}>||
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=0.0 height=0.0>
                        <Interactive
                            hit_area={beui::Area::Nowhere}
                            on_press_outside={move |pos: Pos2| dismiss_outside(window, pos)}
                        />
                    </CanvasItem>
                </Canvas>
            </Show>
        </Layers>
    }
}

#[component]
fn PluginLayer(mount: u64, id: LayerId, shown: Memo<Vec<Shown>>) -> NodeId {
    let layer = create_memo(move || {
        shown.with(|shown| {
            shown
                .iter()
                .find(|shown| shown.mount == mount && shown.layer.id == id)
                .map(|shown| shown.layer.clone())
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
            for (scrim, color) in &scrims {
                painter.rect_filled(
                    scrim.translate(rect.min.to_vec2()),
                    0.0,
                    beui::color::Color::literal(*color),
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
                .filter(|child| child.in_layer == Some(id))
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
    let blits: LayerBlits = Rc::new(RefCell::new(Vec::new()));
    let held: Rc<RefCell<Option<beui::native::Drawing>>> = Rc::new(RefCell::new(None));
    let pixels = create_memo(move || {
        let owner = owner(mount)?;
        owner.revision.get();
        let drawn = layer.get().map_or_else(Vec::new, |layer| {
            block_app_plugin_host::layer_blits(&owner.plugin, owner.instance, owner.region, &layer)
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
                .get_or_insert_with(|| block_app_plugin_host::layers_drawing(Rc::clone(&blits)))
                .redrawn(damage),
        )
    });
    view! {
        <Layers>
            <Drawing draw={scrim_draw} />
            <LayerChildren keys={below} />
            <Drawing draw={Prop::dynamic(move || draw_gpu(pixels.get()))} />
            <LayerChildren keys={above} />
        </Layers>
    }
}

#[component]
fn LayerChildren(keys: Memo<Vec<(u64, ChildId)>>) -> NodeId {
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
                let rect = create_memo(move || {
                    owner(mount)
                        .filter(|owner| owner.state.is_alive())
                        .map_or(Rect::ZERO, |owner| owner.state.with(|view| view.rect))
                });
                let built = held
                    .and_then(|owner| {
                        let scope = owner.scope.as_ref()?.child()?;
                        let built = scope.run(|| owner.child_view.with(|view| view(id, child, rect)));
                        on_cleanup(move || scope.dispose());
                        Some(built)
                    })
                    .unwrap_or_else(|| view! {
                        <Frame />
                    });
                let x = create_memo(move || clip.get().min.x);
                let y = create_memo(move || clip.get().min.y);
                let width = create_memo(move || clip.get().width());
                let height = create_memo(move || clip.get().height());
                view! {
                    <CanvasItem x={x} y={y} width={width} height={height}>{built}</CanvasItem>
                }
            }</ForEach>
        </Canvas>
    }
}

#[cfg(test)]
mod tests;
