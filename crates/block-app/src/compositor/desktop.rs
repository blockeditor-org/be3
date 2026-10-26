use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use beui::reactive::*;
use beui::{Forwarded, ForwardedInput, NodeId, PointerPhase, Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{
    EditorInstanceId, EditorRegion, WindowEdge, WindowPlacement, WindowRequest,
};

use crate::plugin_host::HostChildStatus;

const OPEN_GRAB: Vec2 = vec2(24.0, 16.0);
const SMALLEST: Vec2 = vec2(96.0, 64.0);
const PLACEMENT: &str = "desktop";

#[derive(Clone, Copy)]
pub(crate) struct DesktopPlace {
    pub(crate) display: Memo<Rect>,
    pub(crate) displays: Memo<Vec<Rect>>,
    pub(crate) scale: Memo<f32>,
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
    pub(crate) window: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Floated {
    pub(crate) key: FloatedKey,
    pub(crate) rect: Rect,
}

impl Floated {
    pub(crate) fn placement(&self) -> String {
        format!("{PLACEMENT} {} {}", self.rect.min.x, self.rect.min.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Move,
    Resize(WindowEdge),
}

#[derive(Clone, Debug, PartialEq)]
struct Gesture {
    key: FloatedKey,
    kind: Kind,
    began_on: Option<usize>,
    start: Option<(Pos2, Rect)>,
}

type StatusKey = (String, EditorInstanceId, EditorRegion);

thread_local! {
    static FLOATED: Cell<Option<Signal<Vec<Floated>>>> = const { Cell::new(None) };
    static GESTURE: Cell<Option<Signal<Option<Gesture>>>> = const { Cell::new(None) };
    static PENDING: RefCell<Vec<(String, WindowRequest)>> = const { RefCell::new(Vec::new()) };
    static PRESSED: RefCell<Option<(FloatedKey, Pos2, Rect)>> = const { RefCell::new(None) };
    static FORGOTTEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static POINTER: Cell<Pos2> = const { Cell::new(Pos2::ZERO) };
    static ORIGINS: RefCell<HashMap<(String, EditorInstanceId, EditorRegion), Pos2>> =
        RefCell::new(HashMap::new());
    static CATCHERS: RefCell<HashMap<NodeId, Catcher>> = RefCell::new(HashMap::new());
    static STATUSES: RefCell<HashMap<StatusKey, BTreeMap<u64, Vec<HostChildStatus>>>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn install() {
    FLOATED.set(Some(create_signal(Vec::new())));
    GESTURE.set(Some(create_signal(None)));
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

pub(crate) fn request(plugin: &str, requests: Vec<WindowRequest>) {
    if requests.is_empty() {
        return;
    }
    PENDING.with(|pending| {
        pending
            .borrow_mut()
            .extend(requests.into_iter().map(|request| (plugin.to_owned(), request)))
    });
    crate::host::request_repaint();
}

pub(crate) fn forget(plugin: &str) {
    FORGOTTEN.with(|forgotten| forgotten.borrow_mut().push(plugin.to_owned()));
    crate::host::request_repaint();
}

pub(crate) fn note_origin(plugin: &str, instance: EditorInstanceId, region: EditorRegion, origin: Pos2) {
    ORIGINS.with(|origins| {
        origins
            .borrow_mut()
            .insert((plugin.to_owned(), instance, region), origin)
    });
}

pub(crate) fn forget_origin(plugin: &str, instance: EditorInstanceId, region: EditorRegion) {
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
    pub(crate) region: EditorRegion,
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
    let pending = PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    let forgotten = FORGOTTEN.with(|forgotten| std::mem::take(&mut *forgotten.borrow_mut()));
    if (pending.is_empty() && forgotten.is_empty()) || FLOATED.get().is_none() {
        return;
    }
    let floated = floated_signal();
    let mut windows = floated.get_untracked();
    windows.retain(|window| !forgotten.contains(&window.key.plugin));
    let mut gesture = None;
    for (plugin, request) in pending {
        let key = |window: u64, instance: EditorInstanceId| FloatedKey {
            plugin: plugin.clone(),
            instance,
            window,
        };
        let find = |windows: &[Floated], window: u64| {
            windows
                .iter()
                .position(|held| held.key.plugin == plugin && held.key.window == window)
        };
        match request {
            WindowRequest::Open(open) => {
                let size = vec2(open.size.width.max(1.0), open.size.height.max(1.0));
                let origin = match open.placement {
                    WindowPlacement::Over(over) => {
                        origin_of(&plugin, over.region.instance, over.region.region, &windows)
                            + over.at.to_vec2()
                    }
                    WindowPlacement::Host(placement) => {
                        read_placement(&placement).unwrap_or_else(|| POINTER.get() - OPEN_GRAB)
                    }
                    WindowPlacement::AtPointer => POINTER.get() - OPEN_GRAB,
                };
                let window = Floated {
                    key: key(open.window, open.opener.instance),
                    rect: Rect::from_min_size(origin, size),
                };
                match find(&windows, open.window) {
                    Some(index) => windows[index] = window,
                    None => windows.push(window),
                }
            }
            WindowRequest::BeginMove(window) => {
                if let Some(index) = find(&windows, window) {
                    gesture = Some(Gesture {
                        key: windows[index].key.clone(),
                        kind: Kind::Move,
                        began_on: None,
                        start: pressed_on(&windows[index].key),
                    });
                }
            }
            WindowRequest::BeginResize(resize) => {
                if let Some(index) = find(&windows, resize.window) {
                    gesture = Some(Gesture {
                        key: windows[index].key.clone(),
                        kind: Kind::Resize(resize.edge),
                        began_on: None,
                        start: pressed_on(&windows[index].key),
                    });
                }
            }
            WindowRequest::Resize(resize) => {
                if let Some(index) = find(&windows, resize.window) {
                    let held = &mut windows[index];
                    held.rect = Rect::from_min_size(
                        held.rect.min,
                        vec2(resize.size.width.max(1.0), resize.size.height.max(1.0)),
                    );
                }
            }
            WindowRequest::Close(window) => {
                if let Some(index) = find(&windows, window) {
                    windows.remove(index);
                }
            }
        }
    }
    if floated.with_untracked(|held| *held != windows) {
        floated.set(windows);
    }
    if let Some(mut gesture) = gesture {
        gesture.began_on = owner_index(&gesture.key);
        gesture_signal().set(Some(gesture));
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
    crate::display::shown().with_untracked(|shown| {
        shown.iter().map(|display| display.rect).collect()
    })
}

fn origin_of(
    plugin: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    windows: &[Floated],
) -> Pos2 {
    if let EditorRegion::Window(window) = region
        && let Some(held) = windows
            .iter()
            .find(|held| held.key.plugin == plugin && held.key.window == window)
    {
        return held.rect.min;
    }
    ORIGINS
        .with(|origins| origins.borrow().get(&(plugin.to_owned(), instance, region)).copied())
        .unwrap_or(Pos2::ZERO)
}

fn read_placement(placement: &str) -> Option<Pos2> {
    let mut words = placement.split_whitespace();
    if words.next()? != PLACEMENT {
        return None;
    }
    let x = words.next()?.parse::<f32>().ok()?;
    let y = words.next()?.parse::<f32>().ok()?;
    (x.is_finite() && y.is_finite()).then(|| pos2(x, y))
}

pub(crate) fn placement_of(key: &FloatedKey) -> Option<String> {
    floated_signal().with_untracked(|held| {
        held.iter()
            .find(|window| window.key == *key)
            .map(Floated::placement)
    })
}

pub(crate) fn rect_of(key: FloatedKey) -> Memo<Option<Rect>> {
    let floated = floated();
    create_memo(move || {
        floated.with(|held| {
            held.iter()
                .find(|window| window.key == key)
                .map(|window| window.rect)
        })
    })
}

pub(crate) fn shown_on(
    place: DesktopPlace,
    plugin: String,
    instance: EditorInstanceId,
) -> Memo<Vec<u64>> {
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
                .filter(|window| {
                    let owner = crate::display::owner(&displays, window.rect);
                    let moving = gesture
                        .as_ref()
                        .filter(|gesture| gesture.key == window.key);
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
        let moved = at - start;
        let rect = match held.kind {
            Kind::Move => start_rect.translate(moved),
            Kind::Resize(edge) => resized(start_rect, edge, moved),
        };
        floated.update(|windows| {
            if let Some(window) = windows.iter_mut().find(|window| window.key == *key) {
                window.rect = rect;
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

fn resized(rect: Rect, edge: WindowEdge, moved: Vec2) -> Rect {
    let (mut min, mut max) = (rect.min, rect.max);
    let (left, right, top, bottom) = match edge {
        WindowEdge::Top => (false, false, true, false),
        WindowEdge::Bottom => (false, false, false, true),
        WindowEdge::Left => (true, false, false, false),
        WindowEdge::Right => (false, true, false, false),
        WindowEdge::TopLeft => (true, false, true, false),
        WindowEdge::TopRight => (false, true, true, false),
        WindowEdge::BottomLeft => (true, false, false, true),
        WindowEdge::BottomRight => (false, true, false, true),
    };
    if left {
        min.x = (min.x + moved.x).min(max.x - SMALLEST.x);
    }
    if right {
        max.x = (max.x + moved.x).max(min.x + SMALLEST.x);
    }
    if top {
        min.y = (min.y + moved.y).min(max.y - SMALLEST.y);
    }
    if bottom {
        max.y = (max.y + moved.y).max(min.y + SMALLEST.y);
    }
    Rect::from_min_max(min, max)
}

fn raise(key: &FloatedKey) {
    let floated = floated_signal();
    let last = floated.with_untracked(|windows| windows.last().map(|window| window.key.clone()));
    if last.as_ref() == Some(key) {
        return;
    }
    floated.update(|windows| {
        if let Some(index) = windows.iter().position(|window| window.key == *key) {
            let window = windows.remove(index);
            windows.push(window);
        }
    });
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
