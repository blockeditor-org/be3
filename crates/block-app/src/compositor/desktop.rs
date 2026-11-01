use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use beui::animation::{create_animation, motion};
use beui::reactive::*;
use beui::unstyled::rubber_band::{Band, WINDOW_SPRING};
use beui::{Forwarded, ForwardedInput, NodeId, PointerPhase, Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{
    EditorInstanceId, HostView, ParentView, View, ViewId, ViewPlacement, ViewRef, WindowEdge,
    WindowOpen, WindowPlacement,
};

use block_app_plugin_host::HostChildStatus;
use window_stack::{Gesture as Kind, Host, Stacked};

const SHRINK_TO: f32 = 160.0;
const GRAB_MARGIN: f32 = 24.0;
const KEEP_VISIBLE: f32 = 96.0;

#[derive(Clone, Copy)]
pub(crate) struct DesktopPlace {
    pub(crate) display: Memo<Rect>,
    pub(crate) displays: Memo<Vec<Rect>>,
}

impl DesktopPlace {
    fn index(self) -> Option<usize> {
        let display = self.display.get();
        self.displays
            .with(|displays| displays.iter().position(|held| *held == display))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FloatedKey {
    pub(crate) plugin: String,
    pub(crate) instance: EditorInstanceId,
    pub(crate) window: ViewId,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Floated {
    pub(crate) key: FloatedKey,
    pub(crate) rect: Rect,
    pub(crate) panel: Option<Panel>,
    pub(crate) anchor: View,
    pub(crate) parent: Option<ViewId>,
    pub(crate) grab: Option<Vec2>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Panel {
    pub(crate) opened: WindowPlacement,
    pub(crate) placed: Option<String>,
    pub(crate) moves: u32,
    pub(crate) resizes: (u32, WindowEdge),
}

impl Floated {
    fn anchored(&self, region: View) -> bool {
        match region.placement {
            ViewPlacement::Dock => self.anchor == region,
            ViewPlacement::Host(HostView::Window) => false,
            _ => self.anchor.placement != ViewPlacement::Dock,
        }
    }

    fn overlaid_in(&self, region: View) -> bool {
        match (region.is_window(), self.parent) {
            (true, parent) => parent == Some(region.id),
            (false, Some(_)) => false,
            (false, None) => self.anchored(region),
        }
    }

    pub(crate) fn placement(&self) -> Option<String> {
        match &self.panel {
            Some(panel) => panel.placed.clone(),
            None => Some(window_stack::placement(self.rect.min)),
        }
    }
}

impl Stacked for Floated {
    fn view(&self) -> ViewId {
        self.key.window
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn opened_over(&self, other: &Self) -> bool {
        self.key.plugin == other.key.plugin && self.parent == Some(other.key.window)
    }

    fn pinned(&self) -> bool {
        self.panel.is_some()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Gesture {
    key: FloatedKey,
    kind: Kind,
    began_on: Option<usize>,
    start: Option<(Pos2, Rect)>,
}

type StatusKey = (String, EditorInstanceId, ViewId);

thread_local! {
    static FLOATED: Cell<Option<Signal<Vec<Floated>>>> = const { Cell::new(None) };
    static GESTURE: Cell<Option<Signal<Option<Gesture>>>> = const { Cell::new(None) };
    static WORK_AREAS: Cell<Option<Signal<Vec<(StatusKey, Rect)>>>> = const { Cell::new(None) };
    static PRESSED: RefCell<Option<(FloatedKey, Pos2, Rect)>> = const { RefCell::new(None) };
    static POINTER: Cell<Pos2> = const { Cell::new(Pos2::ZERO) };
    static ORIGINS: RefCell<HashMap<(String, EditorInstanceId, ViewId), Pos2>> =
        RefCell::new(HashMap::new());
    static CATCHERS: RefCell<HashMap<NodeId, Catcher>> = RefCell::new(HashMap::new());
    static SHOWN: RefCell<HashMap<StatusKey, (ViewPlacement, usize)>> =
        RefCell::new(HashMap::new());
    static STATUSES: RefCell<HashMap<StatusKey, BTreeMap<u64, Vec<HostChildStatus>>>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn install() {
    let floated = create_signal(Vec::new());
    let gesture = create_signal(None);
    let areas = create_signal(Vec::new());
    FLOATED.set(Some(floated));
    GESTURE.set(Some(gesture));
    WORK_AREAS.set(Some(areas));
    create_effect(move || {
        areas.with(|_| ());
        let moving = gesture.with(|gesture| gesture.as_ref().map(|gesture| gesture.key.clone()));
        let displays = crate::display::made().map_or_else(Vec::new, |shown| {
            shown.with(|shown| shown.iter().map(|display| display.rect).collect::<Vec<_>>())
        });
        untrack(|| {
            let mut windows = floated.get();
            if keep_in_work_areas(&mut windows, &displays, moving.as_ref()) {
                floated.set(windows);
            }
        });
    });
}

fn work_areas_signal() -> Signal<Vec<(StatusKey, Rect)>> {
    WORK_AREAS
        .get()
        .expect("the compositor is installed while the document is built")
}

pub(crate) fn note_work_area(view: StatusKey, area: Option<Rect>) {
    let Some(areas) = WORK_AREAS.get().filter(|areas| areas.is_alive()) else {
        return;
    };
    let held = areas.with_untracked(|areas| {
        areas
            .iter()
            .find(|(key, _)| *key == view)
            .map(|(_, area)| *area)
    });
    if held == area {
        return;
    }
    areas.update(|areas| {
        areas.retain(|(key, _)| *key != view);
        if let Some(area) = area {
            areas.push((view, area));
        }
    });
}

fn work_area_on(plugin: &str, instance: EditorInstanceId, display: Rect) -> Option<Rect> {
    work_areas_signal().with_untracked(|areas| {
        areas
            .iter()
            .filter(|((held, from, _), _)| held == plugin && *from == instance)
            .map(|(_, area)| (*area, display.intersect(*area)))
            .filter(|(_, shared)| shared.is_positive())
            .max_by(|(_, left), (_, right)| {
                (left.width() * left.height()).total_cmp(&(right.width() * right.height()))
            })
            .map(|(area, _)| area)
    })
}

fn keep_in_work_areas(
    windows: &mut [Floated],
    displays: &[Rect],
    moving: Option<&FloatedKey>,
) -> bool {
    let moving = moving.and_then(|key| windows.iter().position(|window| window.key == *key));
    window_stack::keep_in_work_areas(windows, moving, |window| {
        let display = displays[crate::display::owner(displays, window.rect)?];
        let work = work_area_on(&window.key.plugin, window.key.instance, display)?;
        Some((display, work))
    })
}

fn floated_signal() -> Signal<Vec<Floated>> {
    FLOATED
        .get()
        .expect("the compositor is installed while the document is built")
}

fn gesture_signal() -> Signal<Option<Gesture>> {
    GESTURE
        .get()
        .expect("the compositor is installed while the document is built")
}

pub(crate) fn floated() -> ReadSignal<Vec<Floated>> {
    floated_signal().read_only()
}

pub(crate) fn place() -> Option<DesktopPlace> {
    use_context::<DesktopPlace>()
}

pub(crate) fn note_origin(plugin: &str, instance: EditorInstanceId, region: ViewId, origin: Pos2) {
    ORIGINS.with(|origins| {
        origins
            .borrow_mut()
            .insert((plugin.to_owned(), instance, region), origin)
    });
}

pub(crate) fn show_view(plugin: &str, instance: EditorInstanceId, view: View) {
    SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        let held = shown
            .entry((plugin.to_owned(), instance, view.id))
            .or_insert((view.placement, 0));
        held.1 += 1;
    });
}

pub(crate) fn hide_view(plugin: &str, instance: EditorInstanceId, view: ViewId) {
    SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        let key = (plugin.to_owned(), instance, view);
        if let Some(held) = shown.get_mut(&key) {
            held.1 -= 1;
            if held.1 == 0 {
                shown.remove(&key);
            }
        }
    });
}

fn shown_view(plugin: &str, instance: EditorInstanceId, view: ViewId) -> View {
    let placement = SHOWN.with(|shown| {
        shown
            .borrow()
            .get(&(plugin.to_owned(), instance, view))
            .map(|(placement, _)| *placement)
    });
    View {
        id: view,
        placement: placement.unwrap_or(ViewPlacement::Parent(ParentView::Main)),
    }
}

pub(crate) fn forget_origin(plugin: &str, instance: EditorInstanceId, region: ViewId) {
    ORIGINS.with(|origins| {
        origins
            .borrow_mut()
            .remove(&(plugin.to_owned(), instance, region))
    });
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Catcher {
    pub(crate) plugin: String,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: ViewId,
    pub(crate) zoom: f32,
    pub(crate) rect: Rect,
    pub(crate) parent: Option<NodeId>,
}

pub(crate) fn register_catcher(node: NodeId, catcher: Catcher) {
    CATCHERS.with(|catchers| catchers.borrow_mut().insert(node, catcher));
}

pub(crate) fn forget_catcher(node: NodeId) {
    CATCHERS.with(|catchers| catchers.borrow_mut().remove(&node));
}

pub(crate) fn catcher(node: NodeId) -> Option<Catcher> {
    CATCHERS.with(|catchers| catchers.borrow().get(&node).cloned())
}

pub(crate) fn aim(
    plugin: &str,
    own: Option<NodeId>,
    rect: Rect,
    over: Option<beui::Over>,
    pos: Pos2,
) -> beui::Aim {
    if let Some(over) = over {
        let mut node = over.node;
        let mut at = None;
        for _ in 0..16 {
            if Some(node) == own {
                return beui::Aim::Own;
            }
            let Some(held) = catcher(node) else {
                break;
            };
            let absolute = *at.get_or_insert(held.rect.min + over.local.to_vec2());
            if held.plugin == plugin {
                let zoom = match held.zoom.is_finite() && held.zoom > 0.0 {
                    true => held.zoom,
                    false => 1.0,
                };
                return beui::Aim::Other {
                    node,
                    at: Pos2::ZERO + (absolute - held.rect.min) / zoom,
                };
            }
            let Some(parent) = held.parent else {
                break;
            };
            node = parent;
        }
    }
    match rect.contains(pos) {
        true => beui::Aim::Own,
        false => beui::Aim::Away,
    }
}

pub(crate) fn apply() {
    let (pending, forgotten) = block_app_plugin_host::take_window_requests();
    if (pending.is_empty() && forgotten.is_empty()) || FLOATED.get().is_none() {
        return;
    }
    let floated = floated_signal();
    let mut windows = floated.get_untracked();
    windows.retain(|window| !forgotten.contains(&window.key.plugin));
    let mut gesture = None;
    for (plugin, request) in pending {
        let Some((index, kind)) = window_stack::ask(&mut windows, &mut Opener(&plugin), request)
        else {
            continue;
        };
        let window = &mut windows[index];
        match (&mut window.panel, kind) {
            (Some(panel), Kind::Move) => panel.moves += 1,
            (Some(panel), Kind::Resize(edge)) => panel.resizes = (panel.resizes.0 + 1, edge),
            (None, kind) => {
                gesture = Some(Gesture {
                    key: window.key.clone(),
                    kind,
                    began_on: None,
                    start: pressed_on(&window.key),
                });
            }
        }
    }
    let moving = gesture
        .as_ref()
        .map(|gesture: &Gesture| gesture.key.clone())
        .or_else(|| {
            gesture_signal().with_untracked(|held| held.as_ref().map(|held| held.key.clone()))
        });
    let displays = crate::display::made().map_or_else(Vec::new, |shown| {
        shown.with_untracked(|shown| shown.iter().map(|display| display.rect).collect())
    });
    keep_in_work_areas(&mut windows, &displays, moving.as_ref());
    if floated.with_untracked(|held| *held != windows) {
        floated.set(windows);
    }
    if let Some(mut gesture) = gesture {
        gesture.began_on = owner_index(&gesture.key);
        gesture_signal().set(Some(gesture));
    }
}

struct Opener<'a>(&'a str);

impl Host<Floated> for Opener<'_> {
    fn mine(&self, window: &Floated) -> bool {
        window.key.plugin == self.0
    }

    fn pointer(&self) -> Pos2 {
        POINTER.get()
    }

    fn over(&self, view: ViewRef, at: Pos2) -> Pos2 {
        let (plugin, instance, region) = (self.0, view.instance, view.view);
        let desktop = ORIGINS.with(|origins| {
            origins
                .borrow()
                .get(&(plugin.to_owned(), instance, region))
                .copied()
        });
        if let Some(origin) = desktop {
            return origin + at.to_vec2();
        }
        match block_app_plugin_host::view_placement_of(plugin, instance, region) {
            Some(placed) if placed.zoom.is_finite() && placed.zoom > 0.0 => {
                placed.rect.min + at.to_vec2() * placed.zoom
            }
            Some(placed) => placed.rect.min + at.to_vec2(),
            None => at,
        }
    }

    fn open(
        &mut self,
        open: &WindowOpen,
        rect: Rect,
        held: Option<&Floated>,
        opener: Option<&Floated>,
    ) -> Floated {
        let plugin = self.0;
        let panel = block_app_base::host::opens_windows().then(|| Panel {
            opened: open.placement.clone(),
            placed: None,
            moves: 0,
            resizes: (0, WindowEdge::BottomRight),
        });
        let panel = match (panel, held.and_then(|held| held.panel.as_ref())) {
            (Some(panel), Some(was)) => Some(Panel {
                placed: was.placed.clone(),
                moves: was.moves,
                resizes: was.resizes,
                ..panel
            }),
            (panel, _) => panel,
        };
        let opened_from = shown_view(plugin, open.opener.instance, open.opener.view);
        let anchor = match (opener, opened_from.is_window()) {
            (Some(opener), _) => opener.anchor,
            (None, true) => View::MAIN,
            (None, false) => opened_from,
        };
        Floated {
            key: FloatedKey {
                plugin: plugin.to_owned(),
                instance: open.opener.instance,
                window: open.view,
            },
            rect,
            panel,
            anchor,
            parent: opener.map(|opener| opener.key.window),
            grab: None,
        }
    }
}

fn pressed_on(key: &FloatedKey) -> Option<(Pos2, Rect)> {
    PRESSED.with(|pressed| {
        pressed
            .borrow()
            .as_ref()
            .filter(|(held, _, _)| held == key)
            .map(|(_, at, rect)| (*at, *rect))
    })
}

fn owner_index(key: &FloatedKey) -> Option<usize> {
    let rect = floated_signal().with_untracked(|held| {
        held.iter()
            .find(|window| window.key == *key)
            .map(|window| window.rect)
    })?;
    let displays = desktop_displays();
    crate::display::owner(&displays, rect)
}

fn desktop_displays() -> Vec<Rect> {
    crate::display::shown()
        .with_untracked(|shown| shown.iter().map(|display| display.rect).collect())
}

pub(crate) fn clamped(rect: Rect, viewport: Vec2) -> Rect {
    let size = rect.size().min(viewport).max(Vec2::ZERO);
    let x = rect.min.x.min(viewport.x - size.x).max(0.0);
    let room = viewport.y - rect.min.y.max(0.0);
    let (y, height) = match room >= size.y.min(SHRINK_TO) {
        true => (rect.min.y.max(0.0), size.y.min(room)),
        false => ((viewport.y - size.y).max(0.0), size.y),
    };
    Rect::from_min_size(pos2(x, y), vec2(size.x, height))
}

pub(crate) fn reachable(rect: Rect, grab: Vec2, viewport: Vec2) -> Pos2 {
    rect.kept_in(
        vec2(KEEP_VISIBLE, grab.y + GRAB_MARGIN).min(rect.size()),
        viewport,
    )
}

pub(crate) fn carried(key: FloatedKey, viewport: Memo<Vec2>) -> Memo<Rect> {
    let floated = floated();
    let gesture = gesture_signal();
    let held = {
        let key = key.clone();
        create_memo(move || {
            floated.with(|held| {
                held.iter()
                    .find(|window| window.key == key)
                    .map(|window| (window.rect, window.grab))
            })
        })
    };
    let moving = {
        let key = key.clone();
        create_memo(move || {
            gesture.with(|gesture| {
                gesture
                    .as_ref()
                    .is_some_and(|gesture| gesture.key == key && gesture.kind == Kind::Move)
            })
        })
    };
    let band = store(RefCell::new(Band::new(WINDOW_SPRING)));
    let overshoot = create_signal(Vec2::ZERO);
    let key = store(key);
    let place = move |origin: Pos2| {
        floated_signal().update(|held| {
            if let Some(window) = held
                .iter_mut()
                .find(|window| key.with(|key| window.key == *key))
            {
                window.rect = Rect::from_min_size(origin, window.rect.size());
            }
        });
    };
    let bounce = create_animation(
        move |frame| {
            let Some((rect, Some(grab))) = held.get_untracked() else {
                return false;
            };
            let viewport = viewport.get_untracked();
            let limit =
                |origin: Pos2| reachable(Rect::from_min_size(origin, rect.size()), grab, viewport);
            let band = band.value();
            let mut band = band.borrow_mut();
            let mut origin = rect.min;
            if !band.step(frame.now, &mut origin, limit, motion().animates()) {
                return false;
            }
            if origin != rect.min {
                place(origin);
            }
            overshoot.set(band.offset);
            band.moving()
        },
        move || {
            band.value().borrow_mut().settle();
            overshoot.set(Vec2::ZERO);
        },
    );
    let was_moving = Cell::new(false);
    create_effect(move || {
        let now_moving = moving.get();
        if now_moving == was_moving.replace(now_moving) {
            return;
        }
        untrack(|| {
            let viewport = viewport.get_untracked();
            match now_moving {
                true => {
                    bounce.stop();
                    let stretched = band.value().borrow_mut().grab(viewport);
                    if stretched != Vec2::ZERO {
                        key.with(|key| carry_from(key, stretched));
                    }
                }
                false => {
                    if let Some((rect, Some(grab))) = held.get_untracked() {
                        let origin = reachable(rect, grab, viewport);
                        if origin != rect.min {
                            place(origin);
                        }
                    }
                    band.value().borrow_mut().release(now());
                    bounce.start();
                }
            }
        });
    });
    create_effect(move || {
        if !moving.get() {
            return;
        }
        let Some((rect, Some(grab))) = held.get() else {
            return;
        };
        let viewport = viewport.get();
        untrack(|| {
            let origin = reachable(rect, grab, viewport);
            let band = band.value();
            let mut band = band.borrow_mut();
            band.stretch(
                origin,
                rect.min - origin,
                viewport,
                motion().animates(),
                now(),
            );
            overshoot.set(band.offset);
        });
    });
    create_memo(move || {
        let Some((rect, grab)) = held.get() else {
            return Rect::ZERO;
        };
        let viewport = viewport.get();
        match grab {
            Some(grab) => Rect::from_min_size(reachable(rect, grab, viewport), rect.size())
                .translate(overshoot.get()),
            None => clamped(rect, viewport),
        }
    })
}

fn carry_from(key: &FloatedKey, by: Vec2) {
    floated_signal().update(|held| {
        if let Some(window) = held.iter_mut().find(|window| window.key == *key) {
            window.rect = window.rect.translate(by);
        }
    });
    gesture_signal().update(|gesture| {
        if let Some((_, rect)) = gesture
            .as_mut()
            .filter(|gesture| gesture.key == *key)
            .and_then(|gesture| gesture.start.as_mut())
        {
            *rect = rect.translate(by);
        }
    });
}

pub(crate) fn overlaid(
    plugin: String,
    instance: EditorInstanceId,
    region: View,
) -> Memo<Vec<ViewId>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            held.iter()
                .filter(|window| window.key.plugin == plugin && window.key.instance == instance)
                .filter(|window| window.overlaid_in(region))
                .filter(|window| window.panel.is_none())
                .map(|window| window.key.window)
                .collect()
        })
    })
}

pub(crate) fn placement_of(key: &FloatedKey) -> Option<String> {
    floated_signal().with_untracked(|held| {
        held.iter()
            .find(|window| window.key == *key)
            .and_then(Floated::placement)
    })
}

pub(crate) fn placement_memo(key: FloatedKey) -> Memo<Option<String>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            held.iter()
                .find(|window| window.key == key)
                .and_then(Floated::placement)
        })
    })
}

pub(crate) fn origin_memo(key: FloatedKey) -> Memo<Option<Pos2>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            held.iter()
                .find(|window| window.key == key && window.panel.is_none())
                .map(|window| window.rect.min)
        })
    })
}

pub(crate) fn panel_of(key: FloatedKey) -> Memo<Option<(Vec2, Panel)>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            held.iter()
                .find(|window| window.key == key)
                .and_then(|window| Some((window.rect.size(), window.panel.clone()?)))
        })
    })
}

pub(crate) fn panels(
    plugin: String,
    instance: EditorInstanceId,
    region: View,
) -> Memo<Vec<ViewId>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            held.iter()
                .filter(|window| window.key.plugin == plugin && window.key.instance == instance)
                .filter(|window| window.anchored(region))
                .filter(|window| window.panel.is_some())
                .map(|window| window.key.window)
                .collect()
        })
    })
}

pub(crate) fn panel_placed(key: &FloatedKey, placement: String) {
    floated_signal().update(|held| {
        if let Some(panel) = held
            .iter_mut()
            .find(|window| window.key == *key)
            .and_then(|window| window.panel.as_mut())
        {
            panel.placed = Some(placement);
        }
    });
}

pub(crate) fn dismiss(key: &FloatedKey) {
    floated_signal().update(|held| {
        let Some(index) = held.iter().position(|window| window.key == *key) else {
            return;
        };
        let gone = window_stack::family(held, index);
        let mut index = 0;
        held.retain(|_| {
            index += 1;
            !gone.contains(&(index - 1))
        });
    });
}

pub(crate) fn rect_of(key: FloatedKey) -> Memo<Option<Rect>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            let window = held.iter().find(|window| window.key == key)?;
            Some(window.rect)
        })
    })
}

pub(crate) fn shown_on(
    place: DesktopPlace,
    plugin: String,
    instance: EditorInstanceId,
) -> Memo<Vec<ViewId>> {
    let floated = floated();
    let gesture = gesture_signal();
    create_memo(move || {
        let index = place.index();
        let displays = place.displays.get();
        let display = place.display.get();
        let gesture = gesture.get();
        floated.with(|held| {
            held.iter()
                .filter(|window| window.key.plugin == plugin && window.key.instance == instance)
                .filter(|window| window.panel.is_none())
                .filter(|window| {
                    let owner = crate::display::owner(&displays, window.rect);
                    let moving = gesture.as_ref().filter(|gesture| gesture.key == window.key);
                    match moving {
                        Some(gesture) => {
                            display.intersects(window.rect)
                                || (gesture.began_on.is_some() && gesture.began_on == index)
                                || owner == index
                        }
                        None => owner == index || (owner.is_none() && index == Some(0)),
                    }
                })
                .map(|window| window.key.window)
                .collect()
        })
    })
}

pub(crate) fn owns(place: DesktopPlace, key: FloatedKey) -> Memo<bool> {
    let rect = rect_of(key);
    create_memo(move || {
        let Some(rect) = rect.get() else {
            return false;
        };
        let displays = place.displays.get();
        let owner = crate::display::owner(&displays, rect);
        owner == place.index() || (owner.is_none() && place.index() == Some(0))
    })
}

pub(crate) fn follow_pointer(input: &ForwardedInput, origin: Vec2) {
    if let Some(event) = input.pointer_events().last() {
        POINTER.set(event.pos + origin);
    }
}

pub(crate) fn follow(key: &FloatedKey, input: ForwardedInput, origin: Vec2) -> ForwardedInput {
    let floated = floated_signal();
    let gesture = gesture_signal();
    let mut events = Vec::with_capacity(input.events.len());
    for forwarded in input.events {
        let Forwarded::Pointer { event, .. } = &forwarded else {
            events.push(forwarded);
            continue;
        };
        let at = event.pos + origin;
        POINTER.set(at);
        if matches!(event.phase, PointerPhase::Down(_)) {
            raise(key);
            let rect = floated.with_untracked(|windows| {
                windows
                    .iter()
                    .find(|window| window.key == *key)
                    .map(|window| window.rect)
            });
            if let Some(rect) = rect {
                PRESSED.with(|pressed| *pressed.borrow_mut() = Some((key.clone(), at, rect)));
            }
        }
        let held = gesture.with_untracked(|held| held.clone().filter(|held| held.key == *key));
        let Some(mut held) = held else {
            events.push(forwarded);
            continue;
        };
        let Some(rect) = floated.with_untracked(|windows| {
            windows
                .iter()
                .find(|window| window.key == *key)
                .map(|window| window.rect)
        }) else {
            events.push(forwarded);
            continue;
        };
        let (start, start_rect) = *held.start.get_or_insert((at, rect));
        floated.update(|windows| {
            if let Some(index) = windows.iter().position(|window| window.key == *key) {
                window_stack::drag(windows, index, held.kind, start_rect, at - start);
                if held.kind == Kind::Move {
                    windows[index].grab = Some(start - start_rect.min);
                }
            }
        });
        match event.phase {
            PointerPhase::Up(_) | PointerPhase::Cancel | PointerPhase::Gone => {
                gesture.set(None);
                events.push(forwarded);
            }
            _ => gesture.set(Some(held)),
        }
    }
    ForwardedInput { events, ..input }
}

fn raise(key: &FloatedKey) {
    let floated = floated_signal();
    let mut windows = floated.get_untracked();
    if let Some(index) = windows.iter().position(|window| window.key == *key) {
        window_stack::raise(&mut windows, index);
    }
    if floated.with_untracked(|held| *held != windows) {
        floated.set(windows);
    }
}

pub(crate) fn merged_statuses(
    key: StatusKey,
    mount: u64,
    statuses: Option<Vec<HostChildStatus>>,
) -> Vec<HostChildStatus> {
    STATUSES.with(|held| {
        let mut held = held.borrow_mut();
        let mounts = held.entry(key.clone()).or_default();
        match statuses {
            Some(statuses) => {
                mounts.insert(mount, statuses);
            }
            None => {
                mounts.remove(&mount);
            }
        }
        let mut merged: Vec<HostChildStatus> = Vec::new();
        for status in mounts.values().flatten() {
            if !merged.iter().any(|known| known.child == status.child) {
                merged.push(status.clone());
            }
        }
        if mounts.is_empty() {
            held.remove(&key);
        }
        merged
    })
}

#[cfg(test)]
mod tests;
