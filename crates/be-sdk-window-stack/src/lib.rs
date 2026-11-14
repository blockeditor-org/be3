use be_lib_geometry::{pos2, vec2};
use be_sdk_api::{
    Pos2, Rect, Vec2, ViewId, ViewRef, WindowEdge, WindowOpen, WindowPlacement, WindowRequest,
};

pub const OPEN_GRAB: Vec2 = vec2(24.0, 16.0);
pub const SMALLEST: Vec2 = vec2(96.0, 64.0);
/// How much of a window's top-left corner, where its grip is, stays on a work area, so the window
/// can always be grabbed again.
pub const KEPT: Vec2 = vec2(96.0, 48.0);
pub const WALL: f32 = 0.5;
const PLACEMENT: &str = "desktop";

pub trait Stacked {
    fn view(&self) -> ViewId;
    /// Where the window shows.
    fn rect(&self) -> Rect;
    /// Puts the window at `rect`, as the user or its app asked: it shows there and is wanted
    /// there.
    fn set_rect(&mut self, rect: Rect);
    /// Where the window was last put (`set_rect`), which keeping it in the work areas leaves
    /// alone, so it goes back there once the displays have room for it again.
    fn wanted(&self) -> Rect;
    /// Shows the window at `rect` without changing where it is wanted.
    fn show_at(&mut self, rect: Rect);
    fn opened_over(&self, other: &Self) -> bool;
    fn pinned(&self) -> bool {
        false
    }
    fn above(&self) -> bool {
        false
    }
    /// Covers its whole display (`WindowLevel::Fullscreen`), so it is never kept in a work area.
    fn fullscreen(&self) -> bool {
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
            let size = vec2(open.size.x.max(1.0), open.size.y.max(1.0));
            let rect = Rect::from_min_size(point_of(windows, host, open.placement.clone()), size);
            let held = find(windows, host, open.view);
            let opener = find(windows, host, open.opener.view).map(|index| &windows[index]);
            let window = host.open(&open, rect, held.map(|index| &windows[index]), opener);
            match held {
                Some(index) => windows[index] = window,
                None => windows.push(window),
            }
            settle(windows);
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
                let size = vec2(resize.size.x.max(1.0), resize.size.y.max(1.0));
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
    settle(windows);
}

pub fn settle<W: Stacked>(windows: &mut Vec<W>) {
    let above: Vec<usize> = (0..windows.len())
        .filter(|index| windows[*index].above())
        .flat_map(|index| family(windows, index))
        .collect();
    if above.is_empty() {
        return;
    }
    let (on_top, mut below): (Vec<_>, Vec<_>) = std::mem::take(windows)
        .into_iter()
        .enumerate()
        .partition(|(index, _)| above.contains(index));
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

/// Shows each window but the `moving` one where it is wanted, fitted to the work area of the
/// display it is mostly on and with its grip on a work area. `areas` gives, for a window, every
/// display with its work area.
pub fn keep_in_work_areas<W: Stacked>(
    windows: &mut [W],
    moving: Option<usize>,
    areas: impl Fn(&W) -> Vec<(Rect, Rect)>,
) -> bool {
    let mut moved = false;
    for (index, window) in windows.iter_mut().enumerate() {
        if window.pinned() || window.fullscreen() || Some(index) == moving {
            continue;
        }
        let areas = areas(window);
        let wanted = window.wanted();
        let displays: Vec<Rect> = areas.iter().map(|(display, _)| *display).collect();
        let Some((display, work)) = home(&displays, wanted).map(|home| areas[home]) else {
            continue;
        };
        let to = fitted(wanted, display, work);
        let to = Rect::from_min_size(reachable(to, &areas), to.size());
        if to != window.rect() {
            window.show_at(to);
            moved = true;
        }
    }
    moved
}

/// Where `rect` goes so that its grip (`KEPT` of its top-left corner) lies on one of the work
/// areas of `areas` (each display with its work area): where it is when it already does, else
/// the nearest such place on the work area of the display holding most of it.
pub fn reachable(rect: Rect, areas: &[(Rect, Rect)]) -> Pos2 {
    let displays: Vec<Rect> = areas.iter().map(|(display, _)| *display).collect();
    let Some((_, work)) = home(&displays, rect).map(|home| areas[home]) else {
        return rect.min;
    };
    let works: Vec<Rect> = areas.iter().map(|(_, work)| *work).collect();
    grabbable(rect, &works, work)
}

/// Where `rect` goes so that its grip (`KEPT` of its top-left corner) lies on one of `works`:
/// where it is when it already does, else the nearest such place on `home`. The rest of the
/// window may hang off any edge, or over any band.
pub fn grabbable(rect: Rect, works: &[Rect], home: Rect) -> Pos2 {
    let kept = KEPT.min(rect.size());
    let grip = Rect::from_min_size(rect.min, kept);
    let reached = works.iter().any(|work| {
        let shared = work.intersect(grip);
        shared.width() >= kept.x - WALL && shared.height() >= kept.y - WALL
    });
    if reached {
        return rect.min;
    }
    let x = rect.min.x.min(home.max.x - kept.x).max(home.min.x);
    let y = rect.min.y.min(home.max.y - kept.y).max(home.min.y);
    pos2(x, y)
}

/// `rect` shrunk to `work` on each axis a band insets, or brought wholly into `work` when it is on
/// no part of `display`. Where it shows on its display is left to `reachable`.
pub fn fitted(rect: Rect, display: Rect, work: Rect) -> Rect {
    let shared = display.intersect(rect);
    if shared.width() <= 0.0 || shared.height() <= 0.0 {
        let size = rect.size().min(work.size()).max(Vec2::ZERO);
        let x = rect.min.x.min(work.max.x - size.x).max(work.min.x);
        let y = rect.min.y.min(work.max.y - size.y).max(work.min.y);
        return Rect::from_min_size(pos2(x, y), size);
    }
    let walls_x = work.min.x > display.min.x + WALL || work.max.x < display.max.x - WALL;
    let walls_y = work.min.y > display.min.y + WALL || work.max.y < display.max.y - WALL;
    let mut size = rect.size();
    if walls_x && work.width() > 0.0 {
        size.x = size.x.min(work.width());
    }
    if walls_y && work.height() > 0.0 {
        size.y = size.y.min(work.height());
    }
    Rect::from_min_size(rect.min, size)
}

pub fn home(displays: &[Rect], rect: Rect) -> Option<usize> {
    let shared = |display: &Rect| {
        let shared = display.intersect(rect);
        shared.width().max(0.0) * shared.height().max(0.0)
    };
    let gap = |display: &Rect| {
        let x = (display.min.x - rect.max.x)
            .max(rect.min.x - display.max.x)
            .max(0.0);
        let y = (display.min.y - rect.max.y)
            .max(rect.min.y - display.max.y)
            .max(0.0);
        x * x + y * y
    };
    displays
        .iter()
        .enumerate()
        .filter(|(_, display)| shared(display) > 0.0)
        .max_by(|(_, left), (_, right)| shared(left).total_cmp(&shared(right)))
        .or_else(|| {
            displays
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| gap(left).total_cmp(&gap(right)))
        })
        .map(|(index, _)| index)
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
