use be_lib_stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use be_sdk::{Claim, FrameSpec, ViewId, ViewPlacement, ViewRef};
use beui::window::{HostPlacement, WindowPlacement};
use beui::{Rect, Vec2};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: ViewRef,
    pub placement: ViewPlacement,
    pub block: Option<Uuid>,
    pub size: Vec2,
    pub scale_factor: f32,
    pub ratio: f32,
    pub spec: FrameSpec,
    pub docked: bool,
    pub fit: bool,
}

impl View {
    pub fn view(&self) -> be_sdk::View {
        be_sdk::View {
            id: self.id.view,
            placement: self.placement,
        }
    }

    pub fn chrome_drawn(&self) -> bool {
        self.spec.chrome == be_sdk::FrameChrome::Drawn
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub content: Option<Rect>,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub floating_painted: Vec<Rect>,
    pub marks: Vec<Rect>,
    pub marks_painted: Vec<Rect>,
    pub scrims: Vec<beui::layer::Scrim>,
    pub drawing: Vec<Rect>,
    pub claims: Vec<Claim>,
    pub work_area: Option<Rect>,
    pub fitted: Option<Vec2>,
    /// The view leaves holes that show what is behind it (an editor that took its placer's frame
    /// shows the placer's world around its content), so the host must not back it with its
    /// clear color.
    pub shows_through: bool,
}

#[derive(Default)]
struct Shared {
    views: RefCell<Vec<View>>,
    watchers: RefCell<Vec<Rc<dyn Fn()>>>,
    reports: RefCell<HashMap<ViewRef, Report>>,
    floated: RefCell<Vec<(ViewRef, beui::WindowId)>>,
    keyboard: RefCell<Vec<ViewRef>>,
}

#[derive(Clone, Default)]
pub struct Views(Rc<Shared>);

impl Views {
    pub fn views(&self) -> Vec<View> {
        self.0.views.borrow().clone()
    }

    pub fn view(&self, id: ViewRef) -> Option<View> {
        self.0
            .views
            .borrow()
            .iter()
            .find(|view| view.id == id)
            .cloned()
    }

    pub fn watch(&self, watcher: impl Fn() + 'static) {
        self.0.watchers.borrow_mut().push(Rc::new(watcher));
    }

    pub fn placement(&self, id: ViewRef) -> WindowPlacement {
        WindowPlacement::Host(placement_of(id))
    }

    pub fn floated(&self) -> Vec<(ViewRef, beui::WindowId)> {
        self.0.floated.borrow().clone()
    }

    pub fn set_floated(&self, floated: Vec<(ViewRef, beui::WindowId)>) {
        *self.0.floated.borrow_mut() = floated;
    }

    /// The views the host gave the keyboard, which is how an editor knows its own view has it
    /// rather than another of the app's (`Editor::has_keyboard`).
    pub fn keyboard(&self) -> Vec<ViewRef> {
        self.0.keyboard.borrow().clone()
    }

    pub fn set_keyboard(&self, keyboard: Vec<ViewRef>) {
        if *self.0.keyboard.borrow() == keyboard {
            return;
        }
        *self.0.keyboard.borrow_mut() = keyboard;
        let watchers = self.0.watchers.borrow().clone();
        for watcher in watchers {
            watcher();
        }
    }

    pub fn report(&self, id: ViewRef, report: Report) {
        self.0.reports.borrow_mut().insert(id, report);
    }

    pub fn set(&self, views: Vec<View>) {
        if *self.0.views.borrow() == views {
            return;
        }
        *self.0.views.borrow_mut() = views;
        let watchers = self.0.watchers.borrow().clone();
        for watcher in watchers {
            watcher();
        }
    }

    pub fn take_report(&self, id: ViewRef) -> Report {
        self.0.reports.borrow_mut().remove(&id).unwrap_or_default()
    }

    pub fn placed(&self, placement: &HostPlacement) -> Option<ViewRef> {
        self.0
            .views
            .borrow()
            .iter()
            .map(|view| view.id)
            .find(|id| placement_of(*id) == *placement)
    }
}

fn placement_of(id: ViewRef) -> HostPlacement {
    HostPlacement(format!("app view {} {}", id.instance.0, id.view.0))
}

const WINDOW_VIEWS: u64 = 1 << 61;
const ANCHORED_VIEWS: u64 = 1 << 60;

pub fn window_view(window: beui::WindowId) -> ViewId {
    ViewId(WINDOW_VIEWS | window.raw())
}

pub fn anchored_view(layer: u64) -> ViewId {
    ViewId(ANCHORED_VIEWS | layer)
}

pub fn anchored_layer(view: ViewId) -> Option<u64> {
    (view.0 & !(ANCHORED_VIEWS - 1) == ANCHORED_VIEWS).then_some(view.0 & (ANCHORED_VIEWS - 1))
}
