use beui::reactive::*;
use beui::styled::*;
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::{HostWindow, HostWindowId};

pub(crate) fn create() {}

pub(crate) fn start(_setup: &Setup) {}

pub(crate) fn environment() -> Vec<(String, String)> {
    Vec::new()
}

pub(crate) fn running() -> bool {
    false
}

pub(crate) fn before(_context: &Context, _rect: Rect, _document: &mut Document) {}

pub(crate) fn after(_context: &Context, _document: &mut Document) {}

pub(crate) fn replace_gpu(_setup: &Setup) {}

pub(crate) fn exiting() {}

pub(crate) fn set_owned(_owned: bool) {}

pub(crate) fn revision() -> u64 {
    0
}

pub(crate) fn listed(_hidden: &[HostWindowId]) -> Vec<HostWindow> {
    Vec::new()
}

pub(crate) fn opening_windows() -> Vec<(uuid::Uuid, HostWindowId)> {
    Vec::new()
}

pub(crate) fn close(_window: HostWindowId) {}

pub(crate) fn fullscreen(_window: HostWindowId, _fullscreen: bool) {}

pub(crate) fn focus(_window: HostWindowId) {}

pub(crate) fn launch(_command: String) -> bool {
    false
}

pub(crate) fn exec(_arguments: Vec<String>, _launch: Option<u64>) -> bool {
    false
}

#[component]
pub(crate) fn WindowActions() -> NodeId {
    view! {
        <Frame />
    }
}

#[component]
pub(crate) fn WindowSurface(window: HostWindowId) -> NodeId {
    let _ = window;
    view! {
        <Frame />
    }
}

pub(crate) struct Opener {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) extensions: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Programs;

impl Programs {
    pub(crate) fn scanned(&self) -> bool {
        false
    }

    pub(crate) fn openers(&self) -> Vec<Opener> {
        Vec::new()
    }

    pub(crate) fn open_file(
        &mut self,
        _key: &str,
        _file: &std::path::Path,
        _block: uuid::Uuid,
    ) -> Result<(), String> {
        Err("Programs cannot run here.".to_owned())
    }

    pub(crate) fn scan(&mut self, _pixels: u32) {}

    pub(crate) fn receive(&mut self) -> bool {
        false
    }

    pub(crate) fn listed(&self) -> Vec<block_plugin_api::HostProgram> {
        Vec::new()
    }

    pub(crate) fn launch(&mut self, _key: &str, _launch: Option<u64>) -> bool {
        false
    }
}
