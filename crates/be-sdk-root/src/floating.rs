//! The windows an app opens from a view the root asks with windows (`screen-request.windows`),
//! which the root floats over that view as be-host's desktop floats them
//! (be-sdk-window-stack): each is a view of the instance it opened from, asked for at its size
//! (`HostView::Window`) once it opens (`window-request.open`), where its placement says (at the
//! pointer, or over its opener), above its opener in the stack, until it closes.

use std::cell::Cell;

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_sdk_api::{
    AnchorSpace, EditorInstanceId, HostTransform, HostView, ScreenId, ScreenRequest, ViewId,
    ViewPlacement, ViewRef, ViewportMetrics, WindowLevel, WindowOpen, WindowRequest,
};
use be_sdk_window_stack::{Gesture, Host, Stacked};

thread_local! {
    /// Where the pointer last was in the root's main window, in its points: where a window
    /// opened at the pointer opens.
    static POINTER: Cell<Pos2> = const { Cell::new(Pos2::ZERO) };
}

/// Notes where the pointer is in the root's main window, in its points.
pub(crate) fn note_pointer(at: Pos2) {
    POINTER.set(at);
}

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
    /// The window it opened over, if it opened from one.
    parent: Option<ViewId>,
    level: WindowLevel,
    /// The pixels per point it is laid out at: its opener's.
    raster: f32,
    /// Where it may anchor views, in the root's window's points: its opener's bounds.
    bounds: Rect,
}

impl Floating {
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
            host_place: None,
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
        POINTER.get()
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

/// Takes in `requests`, the window requests a layout commit of the app made, over its `windows`,
/// bottom first, with the `screens` it was asked for: a window opens (only from a screen asked
/// with windows), is placed, resized, raised or closed, or begins to move or be resized with the
/// pointer from where it was last `pressed` (else from where the pointer is). Answers whether any
/// changed, and the move or resize begun.
pub(crate) fn hear(
    windows: &mut Vec<Floating>,
    screens: &[ScreenRequest],
    requests: Vec<WindowRequest>,
    pressed: Option<Pressed>,
) -> (bool, Option<Held>) {
    let before = windows.clone();
    let mut opener = Opener { screens };
    let mut held = None;
    for request in requests {
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
                .map_or((POINTER.get(), window.rect), |pressed| {
                    (pressed.at, pressed.rect)
                });
            held = Some(Held {
                view: window.view,
                gesture,
                start,
                from,
            });
        }
    }
    // A window keeps a size it can be laid out at.
    for window in windows.iter_mut() {
        let size = vec2(window.rect.width().max(1.0), window.rect.height().max(1.0));
        window.rect = Rect::from_min_size(window.rect.min, size);
    }
    (*windows != before, held)
}

/// A press on the window showing `screen`, if one does: it comes in front, with the windows
/// opened over it, as be-host's desktop raises one pressed; a move or resize it asks for next
/// starts from where the pointer is now.
pub(crate) fn press(windows: &mut Vec<Floating>, screen: ScreenId) -> Option<Pressed> {
    let index = windows.iter().position(|window| window.screen == screen)?;
    let pressed = Pressed {
        view: windows[index].view,
        at: POINTER.get(),
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
    // A window resized keeps a size it can be laid out at.
    let window = &mut windows[index];
    let size = vec2(window.rect.width().max(1.0), window.rect.height().max(1.0));
    window.rect = Rect::from_min_size(window.rect.min, size);
    windows.iter().map(|window| window.rect).ne(before)
}

#[cfg(test)]
mod tests;
