use std::cell::RefCell;
use std::rc::Rc;

use beui::{HostPlacement, Pos2, Rect, Vec2, WindowPlacement};
use beui_types::Image;

#[derive(Clone, Debug, PartialEq)]
/// A display on the desktop. `id` is stable while the monitor stays plugged in;
/// `position` and `size` are in the desktop's points, `size` being the physical
/// mode over `scale`.
pub struct Display {
    pub id: String,
    pub name: String,
    pub position: Pos2,
    pub size: Vec2,
    pub scale: f32,
}

impl Display {
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.position, self.size)
    }
}

#[derive(Default)]
struct Shared {
    displays: RefCell<Vec<Display>>,
    watchers: RefCell<Vec<Rc<dyn Fn()>>>,
    screenshot: RefCell<Option<Rc<dyn Fn() -> Option<Image>>>>,
}

#[derive(Clone, Default)]
/// The displays on the desktop, handed to the program through `Setup`. The
/// platform opens no main window: the program opens one `Window` per display at
/// `placement(id)` and the display's size, and any other `Window` is an overlay
/// in the window it was opened in. Mirrors and displays turned off are not listed.
pub struct Displays(Rc<Shared>);

impl Displays {
    pub fn new(displays: Vec<Display>) -> Self {
        let handle = Self::default();
        handle.set(displays);
        handle
    }

    pub fn displays(&self) -> Vec<Display> {
        self.0.displays.borrow().clone()
    }

    pub fn display(&self, id: &str) -> Option<Display> {
        self.0
            .displays
            .borrow()
            .iter()
            .find(|display| display.id == id)
            .cloned()
    }

    /// Hears every change (plugged, unplugged, a new mode, rotation or scale) before
    /// the frame it reaches lays out.
    pub fn watch(&self, watcher: impl Fn() + 'static) {
        self.0.watchers.borrow_mut().push(Rc::new(watcher));
    }

    /// The whole desktop as its displays last showed it, without the pointer, at the
    /// largest display scale, with transparency where no display reaches; `None`
    /// when no display has drawn or the GPU could not read one back.
    pub fn screenshot(&self) -> Option<Image> {
        let take = self.0.screenshot.borrow().clone()?;
        take()
    }

    pub(crate) fn take_screenshots_with(&self, take: impl Fn() -> Option<Image> + 'static) {
        *self.0.screenshot.borrow_mut() = Some(Rc::new(take));
    }

    pub fn placement(&self, id: &str) -> WindowPlacement {
        WindowPlacement::Host(placement_of(id))
    }

    pub fn placed(&self, placement: &HostPlacement) -> Option<String> {
        self.0
            .displays
            .borrow()
            .iter()
            .find(|display| placement_of(&display.id) == *placement)
            .map(|display| display.id.clone())
    }

    pub fn set(&self, displays: Vec<Display>) {
        if *self.0.displays.borrow() == displays {
            return;
        }
        *self.0.displays.borrow_mut() = displays;
        let watchers = self.0.watchers.borrow().clone();
        for watcher in watchers {
            watcher();
        }
    }
}

fn placement_of(id: &str) -> HostPlacement {
    HostPlacement(format!("drm display {id}"))
}
