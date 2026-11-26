//! The windows an app opens from a view the root asks with windows (`screen-request.windows`),
//! which the root floats over that view as be-host's desktop floats them
//! (be-sdk-window-stack): each is a view of the instance it opened from, asked for at its size
//! (`HostView::Window`) once it opens (`window-request.open`), where its placement says (at the
//! pointer, or over its opener), above its opener in the stack, until it closes.

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_sdk_api::{
    AnchorSpace, EditorInstanceId, HostTransform, HostView, ScreenId, ScreenRequest, ViewId,
    ViewPlacement, ViewRef, ViewportMetrics, WindowLevel, WindowOpen, WindowRequest,
};
use be_sdk_window_stack::{Gesture, Host, Stacked};

/// A window an app opened that the root floats.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Floating {
    /// The instance it is a view of, the one it opened from, and its view.
    pub instance: EditorInstanceId,
    pub view: ViewId,
    /// The screen the app opened it with, which the root asks for.
    pub screen: ScreenId,
    /// Where it shows, in the root's window's points.
    pub rect: Rect,
    /// Where it was last put, which keeping it on the window leaves alone.
    wanted: Rect,
    /// The view it opened from, whose request its pixels per point and bounds follow.
    opener: ViewRef,
    /// The window it opened over, if it opened from one.
    parent: Option<ViewId>,
    level: WindowLevel,
    /// The pixels per point it is laid out at: its opener's.
    raster: f32,
    /// Where it may anchor views, in the root's window's points: its opener's bounds.
    bounds: Rect,
}

impl Floating {
    /// Follows its opener's request, as it is asked for now: it is laid out at the same pixels
    /// per point, and anchors views within the same bounds.
    fn follow(&mut self, opener: &ScreenRequest) {
        self.raster = opener.metrics.raster.max(f32::EPSILON);
        if let Some(space) = opener.anchor_space {
            self.bounds = space.bounds;
        }
    }

    /// The screen it is asked for: a window (`HostView::Window`) of its size, which opens
    /// windows of its own, anchoring views anywhere its opener may.
    pub(crate) fn request(&self) -> ScreenRequest {
        let size = self.rect.size();
        ScreenRequest {
            screen: self.screen,
            instance: self.instance,
            view: self.view,
            placement: ViewPlacement::Host(HostView::Window),
            frame: None,
            metrics: ViewportMetrics {
                logical: size,
                visible: Pos2::ZERO,
                pixel_width: (size.x * self.raster).ceil().max(1.0) as u32,
                pixel_height: (size.y * self.raster).ceil().max(1.0) as u32,
                scale_factor: self.raster,
                raster: self.raster,
            },
            windows: true,
            // Where it was put, which the app may open it at again (`window-placement.host`).
            host_place: Some(be_sdk_window_stack::placement(self.wanted.min)),
            docked: false,
            anchor_space: Some(AnchorSpace {
                to_host: HostTransform {
                    origin: self.rect.min,
                    scale: vec2(1.0, 1.0),
                },
                bounds: self.bounds,
            }),
            fit: false,
            origin: None,
        }
    }
}

impl Stacked for Floating {
    fn view(&self) -> ViewId {
        self.view
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.wanted = rect;
    }

    fn wanted(&self) -> Rect {
        self.wanted
    }

    fn show_at(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn opened_over(&self, other: &Self) -> bool {
        self.instance == other.instance && self.parent == Some(other.view)
    }

    fn above(&self) -> bool {
        self.level == WindowLevel::Above
    }

    fn fullscreen(&self) -> bool {
        self.level == WindowLevel::Fullscreen
    }
}

/// Opens an app's windows: from the screens it is asked for, which name where its views are.
struct Opener<'a> {
    screens: &'a [ScreenRequest],
    /// Where the pointer is in the root's main window, in its points.
    pointer: Pos2,
}

impl Opener<'_> {
    fn screen(&self, view: ViewRef) -> Option<&ScreenRequest> {
        self.screens
            .iter()
            .find(|request| (request.instance, request.view) == (view.instance, view.view))
    }
}

impl Host<Floating> for Opener<'_> {
    fn mine(&self, _window: &Floating) -> bool {
        true
    }

    fn pointer(&self) -> Pos2 {
        self.pointer
    }

    /// Where `at` in `view`'s points is in the window's: through the space it anchors views
    /// in, which is where it shows.
    fn over(&self, view: ViewRef, at: Pos2) -> Pos2 {
        match self.screen(view).and_then(|request| request.anchor_space) {
            Some(space) => pos2(
                space.to_host.origin.x + at.x * space.to_host.scale.x,
                space.to_host.origin.y + at.y * space.to_host.scale.y,
            ),
            None => at,
        }
    }

    fn open(
        &mut self,
        open: &WindowOpen,
        rect: Rect,
        held: Option<&Floating>,
        opener: Option<&Floating>,
    ) -> Floating {
        let from = self.screen(open.opener);
        let raster = from
            .map(|request| request.metrics.raster)
            .or(held.map(|held| held.raster))
            .unwrap_or(1.0);
        let bounds = from
            .and_then(|request| request.anchor_space)
            .map(|space| space.bounds)
            .or(held.map(|held| held.bounds))
            .unwrap_or(rect);
        Floating {
            instance: open.opener.instance,
            view: open.view,
            screen: open.screen,
            rect,
            wanted: rect,
            opener: open.opener,
            parent: opener.map(|opener| opener.view),
            level: open.level,
            raster: raster.max(f32::EPSILON),
            bounds,
        }
    }
}

/// A press on a window, which a move or resize it asks for next starts from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pressed {
    view: ViewId,
    /// Where the pointer pressed, in the window's points.
    at: Pos2,
    /// Where the window was then.
    rect: Rect,
}

/// A move or resize of a window under way (`window-request.begin-move`, `begin-resize`), which
/// follows the pointer until it is released.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Held {
    view: ViewId,
    gesture: Gesture,
    /// Where the pointer began, and where the window was then.
    start: Pos2,
    from: Rect,
}

impl Held {
    /// The window being moved or resized.
    pub(crate) fn view(&self) -> ViewId {
        self.view
    }
}

/// Takes in `requests`, the window requests a layout commit of the app made, over its `windows`,
/// bottom first, with the `screens` it was asked for: a window opens (only from a screen asked
/// with windows), is placed, resized, raised or closed, or begins to move or be resized with the
/// pointer from where it was last `pressed` (else from where the `pointer` is). Answers whether
/// any changed, and the move or resize begun.
pub(crate) fn hear(
    windows: &mut Vec<Floating>,
    screens: &[ScreenRequest],
    requests: Vec<WindowRequest>,
    pressed: Option<Pressed>,
    pointer: Pos2,
) -> (bool, Option<Held>) {
    if requests.is_empty() {
        return (false, None);
    }
    let before = windows.clone();
    let mut held = None;
    for request in requests {
        // A window opens from any of the screens asked for, its windows' among them.
        let asked: Vec<ScreenRequest> = screens
            .iter()
            .cloned()
            .chain(windows.iter().map(Floating::request))
            .collect();
        let mut opener = Opener {
            screens: &asked,
            pointer,
        };
        if let WindowRequest::Open(open) = &request {
            let windowed = opener
                .screen(open.opener)
                .is_some_and(|request| request.windows);
            if !windowed {
                continue;
            }
        }
        if let Some((index, gesture)) = be_sdk_window_stack::ask(windows, &mut opener, request) {
            let window = &windows[index];
            let (start, from) = pressed
                .filter(|pressed| pressed.view == window.view)
                .map_or((pointer, window.rect), |pressed| (pressed.at, pressed.rect));
            held = Some(Held {
                view: window.view,
                gesture,
                start,
                from,
            });
        }
    }
    for window in windows.iter_mut() {
        keep_a_size(window);
    }
    (*windows != before, held)
}

/// A press on the window showing `screen`, if one does: it comes in front, with the windows
/// opened over it, as be-host's desktop raises one pressed; a move or resize it asks for next
/// starts from where the pointer is now, at `pointer`.
pub(crate) fn press(
    windows: &mut Vec<Floating>,
    screen: ScreenId,
    pointer: Pos2,
) -> Option<Pressed> {
    let index = windows.iter().position(|window| window.screen == screen)?;
    let pressed = Pressed {
        view: windows[index].view,
        at: pointer,
        rect: windows[index].rect,
    };
    be_sdk_window_stack::raise(windows, index);
    Some(pressed)
}

/// Follows `held` to the pointer at `at`: the window moves, with those opened over it, or is
/// resized, by as much as the pointer moved since it began. Answers whether any changed.
pub(crate) fn drag(windows: &mut [Floating], held: Held, at: Pos2) -> bool {
    let Some(index) = windows.iter().position(|window| window.view == held.view) else {
        return false;
    };
    let before: Vec<Rect> = windows.iter().map(|window| window.rect).collect();
    be_sdk_window_stack::drag(windows, index, held.gesture, held.from, at - held.start);
    keep_a_size(&mut windows[index]);
    windows.iter().map(|window| window.rect).ne(before)
}

/// Each of `windows` follows its opener's request among `screens` (the screens asked for and
/// the other windows'), as asked for now (`Floating::follow`).
pub(crate) fn follow_openers(windows: &mut [Floating], screens: &[ScreenRequest]) {
    for index in 0..windows.len() {
        let opener = windows[index].opener;
        let request = screens
            .iter()
            .cloned()
            .chain(windows.iter().map(Floating::request))
            .find(|request| (request.instance, request.view) == (opener.instance, opener.view));
        if let Some(request) = request {
            windows[index].follow(&request);
        }
    }
}

/// Shows each of `windows` but the one `moving` where it is wanted, kept within the work area of
/// its opener (`frame-report.work-area`), as be-host's desktop keeps one clear of its bars: fitted
/// to it where it is inset, its grip on it. `area` gives a window's display (its bounds) and that
/// work area, in the window's points; one it gives none is left alone. Answers whether any
/// moved.
pub(crate) fn keep_in_work_areas(
    windows: &mut [Floating],
    moving: Option<ViewId>,
    area: impl Fn(ViewRef) -> Option<Rect>,
) -> bool {
    let moving = moving.and_then(|view| windows.iter().position(|window| window.view == view));
    be_sdk_window_stack::keep_in_work_areas(windows, moving, |window| {
        let work = area(window.opener).unwrap_or(window.bounds);
        vec![(window.bounds, window.bounds.intersect(work))]
    })
}

/// A window keeps a size it can be laid out at, where it is wanted too.
fn keep_a_size(window: &mut Floating) {
    let size = vec2(window.rect.width().max(1.0), window.rect.height().max(1.0));
    if size != window.rect.size() {
        window.set_rect(Rect::from_min_size(window.rect.min, size));
    }
}

#[cfg(test)]
mod tests;
