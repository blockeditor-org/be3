use stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use beui_core::geometry::{Rect, Vec2};
use beui_core::window::{HostPlacement, WindowPlacement};
use block_editor_plugin::{Claim, FrameSpec, ViewId, ViewPlacement, ViewRef};
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
}

impl View {
    pub fn view(&self) -> block_editor_plugin::View {
        block_editor_plugin::View {
            id: self.id.view,
            placement: self.placement,
        }
    }

    pub fn chrome_drawn(&self) -> bool {
        self.spec.chrome == block_editor_plugin::FrameChrome::Drawn
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub content: Option<Rect>,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub floating_painted: Vec<Rect>,
    pub scrims: Vec<(Rect, beui_core::color::Color32)>,
    pub drawing: Vec<Rect>,
    pub claims: Vec<Claim>,
}

#[derive(Default)]
struct Shared {
    views: RefCell<Vec<View>>,
    watchers: RefCell<Vec<Rc<dyn Fn()>>>,
    reports: RefCell<HashMap<ViewRef, Report>>,
    floated: RefCell<Vec<(ViewRef, beui_core::window::WindowId)>>,
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

    pub fn floated(&self) -> Vec<(ViewRef, beui_core::window::WindowId)> {
        self.0.floated.borrow().clone()
    }

    pub fn set_floated(&self, floated: Vec<(ViewRef, beui_core::window::WindowId)>) {
        *self.0.floated.borrow_mut() = floated;
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
    HostPlacement(format!("plugin view {} {}", id.instance.0, id.view.0))
}

const WINDOW_VIEWS: u64 = 1 << 61;
const ANCHORED_VIEWS: u64 = 1 << 60;

pub fn window_view(window: beui_core::window::WindowId) -> ViewId {
    ViewId(WINDOW_VIEWS | window.raw())
}

pub fn anchored_view(layer: u64) -> ViewId {
    ViewId(ANCHORED_VIEWS | layer)
}

pub fn anchored_layer(view: ViewId) -> Option<u64> {
    (view.0 & !(ANCHORED_VIEWS - 1) == ANCHORED_VIEWS).then_some(view.0 & (ANCHORED_VIEWS - 1))
}
