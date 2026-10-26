use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use beui_core::context::{FrameOutput, WindowOutput};
use beui_core::geometry::Vec2;
use beui_core::runner::{HostView, Hosted, Incoming, WindowHost};
use beui_core::window::{WindowId, WindowReport, WindowRequest};

use crate::surface::Surface;

#[derive(Default)]
pub(crate) struct Windows {
    pub(crate) opens: bool,
    reports: Vec<WindowReport>,
    requests: Vec<WindowRequest>,
    shown: HashMap<WindowId, Shown>,
}

#[derive(Default)]
pub(crate) struct Shown {
    pub(crate) surface: Rc<RefCell<Surface>>,
    pub(crate) output: Option<WindowOutput>,
    size: Option<Vec2>,
    scale: Option<f32>,
}

impl Windows {
    pub(crate) fn report(&mut self, report: WindowReport) {
        if let WindowReport::Closed(window) = report {
            self.shown.remove(&window);
        }
        self.reports.push(report);
    }

    pub(crate) fn take_requests(&mut self) -> Vec<WindowRequest> {
        std::mem::take(&mut self.requests)
    }

    pub(crate) fn shown(&mut self, window: WindowId) -> &mut Shown {
        self.shown.entry(window).or_default()
    }

    pub(crate) fn get(&self, window: WindowId) -> Option<&Shown> {
        self.shown.get(&window)
    }

    pub(crate) fn place(&mut self, window: WindowId, size: Vec2, scale: f32) -> bool {
        let shown = self.shown.entry(window).or_default();
        let mut reports = Vec::new();
        if shown.size != Some(size) {
            shown.size = Some(size);
            reports.push(WindowReport::Resized { window, size });
        }
        if shown.scale != Some(scale) {
            shown.scale = Some(scale);
            reports.push(WindowReport::Rastered {
                window,
                pixels_per_point: scale,
            });
        }
        let reported = !reports.is_empty();
        self.reports.extend(reports);
        reported
    }

    pub(crate) fn pending(&self) -> bool {
        !self.reports.is_empty()
    }
}

pub(crate) struct PluginWindows(pub(crate) Rc<RefCell<Windows>>);

impl WindowHost for PluginWindows {
    fn opens_windows(&self) -> bool {
        self.0.borrow().opens
    }

    fn passes_main_through(&self) -> bool {
        true
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, _view: &HostView) {
        incoming.extend(
            std::mem::take(&mut self.0.borrow_mut().reports)
                .into_iter()
                .map(Incoming::Window),
        );
    }

    fn frame(&mut self, output: &FrameOutput, _view: &HostView) -> Hosted {
        let mut windows = self.0.borrow_mut();
        windows
            .requests
            .extend(output.window_requests.iter().cloned());
        for window in output.windows.iter().filter(|window| !window.id.is_main()) {
            let shown = windows.shown(window.id);
            shown.surface.borrow_mut().note(
                window,
                window.pixels_per_point,
                window.window.appearance.clear_color,
            );
            shown.output = Some(window.clone());
        }
        Hosted {
            window: output.main().clone(),
            repaint_after: Duration::MAX,
        }
    }
}
