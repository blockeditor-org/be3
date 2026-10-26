use std::cell::RefCell;
use std::rc::Rc;

use beui::{HostPlacement, Pos2, Rect, Vec2, WindowPlacement};

#[derive(Clone, Debug, PartialEq)]
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
}

#[derive(Clone, Default)]
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

    pub fn watch(&self, watcher: impl Fn() + 'static) {
        self.0.watchers.borrow_mut().push(Rc::new(watcher));
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
