use block_plugin_api::{
    Pos2, Rect, Vec2, ViewId, ViewRef, WindowEdge, WindowOpen, WindowPlacement, WindowRequest,
};
use geometry::{pos2, vec2};

pub const OPEN_GRAB: Vec2 = vec2(24.0, 16.0);
pub const SMALLEST: Vec2 = vec2(96.0, 64.0);
pub const WALL: f32 = 0.5;
const PLACEMENT: &str = "desktop";

pub trait Stacked {
    fn view(&self) -> ViewId;
    fn rect(&self) -> Rect;
    fn set_rect(&mut self, rect: Rect);
    fn opened_over(&self, other: &Self) -> bool;
    fn pinned(&self) -> bool {
        false
    }
}

pub trait Host<W> {
    fn mine(&self, window: &W) -> bool;
    fn pointer(&self) -> Pos2;
    fn over(&self, view: ViewRef, at: Pos2) -> Pos2;
    fn open(&mut self, open: &WindowOpen, rect: Rect, held: Option<&W>, opener: Option<&W>) -> W;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    Move,
    Resize(WindowEdge),
}

pub fn find<W: Stacked>(windows: &[W], host: &impl Host<W>, view: ViewId) -> Option<usize> {
    windows
        .iter()
        .position(|window| host.mine(window) && window.view() == view)
}

pub fn point_of<W: Stacked>(
    windows: &[W],
    host: &impl Host<W>,
    placement: WindowPlacement,
) -> Pos2 {
    match placement {
        WindowPlacement::Over(over) => match find(windows, host, over.view.view) {
            Some(index) => windows[index].rect().min + over.at.to_vec2(),
            None => host.over(over.view, over.at),
        },
        WindowPlacement::Host(placement) => {
            read_placement(&placement).unwrap_or(host.pointer() - OPEN_GRAB)
        }
        WindowPlacement::AtPointer => host.pointer() - OPEN_GRAB,
    }
}

pub fn ask<W: Stacked>(
    windows: &mut Vec<W>,
    host: &mut impl Host<W>,
    request: WindowRequest,
) -> Option<(usize, Gesture)> {
    match request {
        WindowRequest::Open(open) => {
            let size = vec2(open.size.width.max(1.0), open.size.height.max(1.0));
            let rect = Rect::from_min_size(point_of(windows, host, open.placement.clone()), size);
            let held = find(windows, host, open.view);
            let opener = find(windows, host, open.opener.view).map(|index| &windows[index]);
            let window = host.open(&open, rect, held.map(|index| &windows[index]), opener);
            match held {
                Some(index) => windows[index] = window,
                None => windows.push(window),
            }
            None
        }
        WindowRequest::Place(place) => {
            let to = point_of(windows, host, place.placement);
            if let Some(index) = find(windows, host, place.view) {
                move_in(windows, index, to);
            }
            None
        }
        WindowRequest::Raise(view) => {
            if let Some(index) = find(windows, host, view) {
                raise(windows, index);
            }
            None
        }
        WindowRequest::BeginMove(view) => {
            find(windows, host, view).map(|index| (index, Gesture::Move))
        }
        WindowRequest::BeginResize(resize) => {
            find(windows, host, resize.view).map(|index| (index, Gesture::Resize(resize.edge)))
        }
        WindowRequest::Resize(resize) => {
            if let Some(index) = find(windows, host, resize.view) {
                let size = vec2(resize.size.width.max(1.0), resize.size.height.max(1.0));
                let rect = Rect::from_min_size(windows[index].rect().min, size);
                windows[index].set_rect(rect);
            }
            None
        }
        WindowRequest::Close(view) => {
            if let Some(index) = find(windows, host, view) {
                windows.remove(index);
            }
            None
        }
    }
}

pub fn family<W: Stacked>(windows: &[W], index: usize) -> Vec<usize> {
    let mut family = vec![index];
    let mut at = 0;
    while at < family.len() {
        let parent = &windows[family[at]];
        let children: Vec<usize> = (0..windows.len())
            .filter(|child| !family.contains(child) && windows[*child].opened_over(parent))
            .collect();
        family.extend(children);
        at += 1;
    }
    family
}

pub fn move_in<W: Stacked>(windows: &mut [W], index: usize, to: Pos2) {
    let moved = to - windows[index].rect().min;
    for member in family(windows, index) {
        let window = &mut windows[member];
        if !window.pinned() {
            let rect = window.rect().translate(moved);
            window.set_rect(rect);
        }
    }
}

pub fn raise<W: Stacked>(windows: &mut Vec<W>, index: usize) {
    let raised = family(windows, index);
    let (on_top, mut below): (Vec<_>, Vec<_>) = std::mem::take(windows)
        .into_iter()
        .enumerate()
        .partition(|(index, _)| raised.contains(index));
    below.extend(on_top);
    *windows = below.into_iter().map(|(_, window)| window).collect();
}

pub fn drag<W: Stacked>(
    windows: &mut [W],
    index: usize,
    gesture: Gesture,
    from: Rect,
    moved: Vec2,
) {
    match gesture {
        Gesture::Move => move_in(windows, index, from.min + moved),
        Gesture::Resize(edge) => windows[index].set_rect(resized(from, edge, moved)),
    }
}

pub fn keep_in_work_areas<W: Stacked>(
    windows: &mut [W],
    moving: Option<usize>,
    area: impl Fn(&W) -> Option<(Rect, Rect)>,
) -> bool {
    let mut moved = false;
    for (index, window) in windows.iter_mut().enumerate() {
        if window.pinned() || Some(index) == moving {
            continue;
        }
        let Some((display, work)) = area(window) else {
            continue;
        };
        let rect = window.rect();
        let to = walled(rect, display, work);
        if to != rect.min {
            window.set_rect(Rect::from_min_size(to, rect.size()));
            moved = true;
        }
    }
    moved
}

pub fn walled(rect: Rect, display: Rect, work: Rect) -> Pos2 {
    let mut min = rect.min;
    if work.max.x < display.max.x - WALL {
        min.x = min
            .x
            .min(work.max.x - rect.width())
            .max(work.min.x.min(rect.min.x));
    }
    if work.max.y < display.max.y - WALL {
        min.y = min
            .y
            .min(work.max.y - rect.height())
            .max(work.min.y.min(rect.min.y));
    }
    if work.min.x > display.min.x + WALL {
        min.x = min.x.max(work.min.x);
    }
    if work.min.y > display.min.y + WALL {
        min.y = min.y.max(work.min.y);
    }
    min
}

pub fn resized(rect: Rect, edge: WindowEdge, moved: Vec2) -> Rect {
    let (mut min, mut max) = (rect.min, rect.max);
    match edge {
        WindowEdge::Top | WindowEdge::TopLeft | WindowEdge::TopRight => {
            min.y = (min.y + moved.y).min(max.y - SMALLEST.y);
        }
        WindowEdge::Bottom | WindowEdge::BottomLeft | WindowEdge::BottomRight => {
            max.y = (max.y + moved.y).max(min.y + SMALLEST.y);
        }
        WindowEdge::Left | WindowEdge::Right => {}
    }
    match edge {
        WindowEdge::Left | WindowEdge::TopLeft | WindowEdge::BottomLeft => {
            min.x = (min.x + moved.x).min(max.x - SMALLEST.x);
        }
        WindowEdge::Right | WindowEdge::TopRight | WindowEdge::BottomRight => {
            max.x = (max.x + moved.x).max(min.x + SMALLEST.x);
        }
        WindowEdge::Top | WindowEdge::Bottom => {}
    }
    Rect::from_min_max(min, max)
}

pub fn placement(at: Pos2) -> String {
    format!("{PLACEMENT} {} {}", at.x, at.y)
}

pub fn read_placement(placement: &str) -> Option<Pos2> {
    let mut words = placement.split_whitespace();
    if words.next()? != PLACEMENT {
        return None;
    }
    let x = words.next()?.parse::<f32>().ok()?;
    let y = words.next()?.parse::<f32>().ok()?;
    (x.is_finite() && y.is_finite()).then(|| pos2(x, y))
}

#[cfg(test)]
mod tests;
