use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_core::geometry::Rect;
use block_editor_plugin::{Claim, Region};

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
    region: RefCell<Option<Region>>,
    ratio: Cell<f32>,
    report: RefCell<Report>,
}

#[derive(Clone, Default)]
pub struct PluginView(Rc<Shared>);

impl PluginView {
    pub fn region(&self) -> Option<Region> {
        self.0.region.borrow().clone()
    }

    pub fn ratio(&self) -> f32 {
        self.0.ratio.get()
    }

    pub fn report(&self, report: Report) {
        *self.0.report.borrow_mut() = report;
    }

    pub(crate) fn begin(&self, region: &Region, ratio: f32) {
        *self.0.region.borrow_mut() = Some(region.clone());
        self.0.ratio.set(ratio);
        *self.0.report.borrow_mut() = Report::default();
    }

    pub(crate) fn take_report(&self) -> Report {
        self.0.report.take()
    }
}
