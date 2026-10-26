use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use beui_core::geometry::{Rect, Vec2};
use beui_core::window::{HostPlacement, WindowPlacement};
use block_editor_plugin::{Claim, FrameSpec, RegionId};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: RegionId,
    pub block: Option<Uuid>,
    pub size: Vec2,
    pub scale_factor: f32,
    pub ratio: f32,
    pub spec: FrameSpec,
}

impl View {
    pub fn chrome_drawn(&self) -> bool {
        self.spec.chrome == block_editor_plugin::FrameChrome::Drawn
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub content: Option<Rect>,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub drawing: Vec<Rect>,
    pub every_sample: bool,
    pub claims: Vec<Claim>,
}

#[derive(Default)]
struct Shared {
    views: RefCell<Vec<View>>,
    watchers: RefCell<Vec<Rc<dyn Fn()>>>,
    reports: RefCell<HashMap<RegionId, Report>>,
    floated: RefCell<Vec<(RegionId, beui_core::window::WindowId)>>,
}

#[derive(Clone, Default)]
pub struct Views(Rc<Shared>);

impl Views {
    pub fn views(&self) -> Vec<View> {
        self.0.views.borrow().clone()
    }

    pub fn view(&self, id: RegionId) -> Option<View> {
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

    pub fn placement(&self, id: RegionId) -> WindowPlacement {
        WindowPlacement::Host(placement_of(id))
    }

    pub fn floated(&self) -> Vec<(RegionId, beui_core::window::WindowId)> {
        self.0.floated.borrow().clone()
    }

    pub(crate) fn set_floated(&self, floated: Vec<(RegionId, beui_core::window::WindowId)>) {
        *self.0.floated.borrow_mut() = floated;
    }

    pub fn report(&self, id: RegionId, report: Report) {
        self.0.reports.borrow_mut().insert(id, report);
    }

    pub(crate) fn set(&self, views: Vec<View>) {
        if *self.0.views.borrow() == views {
            return;
        }
        *self.0.views.borrow_mut() = views;
        let watchers = self.0.watchers.borrow().clone();
        for watcher in watchers {
            watcher();
        }
    }

    pub(crate) fn take_report(&self, id: RegionId) -> Report {
        self.0.reports.borrow_mut().remove(&id).unwrap_or_default()
    }

    pub(crate) fn placed(&self, placement: &HostPlacement) -> Option<RegionId> {
        self.0
            .views
            .borrow()
            .iter()
            .map(|view| view.id)
            .find(|id| placement_of(*id) == *placement)
    }
}

fn placement_of(id: RegionId) -> HostPlacement {
    HostPlacement(format!("plugin view {} {:?}", id.instance.0, id.region))
}
