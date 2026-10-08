use beui::reactive::{Frame, component, view};
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::{HostWindow, HostWindowId};

pub(crate) fn create() {}

pub(crate) fn start(_setup: &Setup) {}

pub(crate) fn running() -> bool {
    false
}

pub(crate) fn before(_context: &Context, _rect: Rect, _document: &mut Document) {}

pub(crate) fn after(_context: &Context, _document: &mut Document) {}

pub(crate) fn replace_gpu(_setup: &Setup) {}

pub(crate) fn exiting() {}

pub(crate) fn revision() -> u64 {
    0
}

pub(crate) fn listed() -> Vec<HostWindow> {
    Vec::new()
}

pub(crate) fn close(_window: HostWindowId) {}

pub(crate) fn launch(_command: String) -> bool {
    false
}

#[component]
pub(crate) fn WindowSurface(window: HostWindowId) -> NodeId {
    let _ = window;
    view! {
        <Frame />
    }
}

#[derive(Default)]
pub(crate) struct Programs {
    items: std::rc::Rc<Vec<beui::styled::LauncherItem>>,
}

impl Programs {
    pub(crate) fn scan(&mut self, _pixels: u32) {}

    pub(crate) fn receive(&mut self) -> bool {
        false
    }

    pub(crate) fn items(&self) -> std::rc::Rc<Vec<beui::styled::LauncherItem>> {
        std::rc::Rc::clone(&self.items)
    }

    pub(crate) fn launch(&self, _key: &str) -> bool {
        false
    }
}
