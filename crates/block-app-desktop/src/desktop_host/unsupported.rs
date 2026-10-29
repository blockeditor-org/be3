use beui::native::Document;
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Rect, native::Context, native::Setup};
use block_plugin_api::{HostWindow, HostWindowId};

pub fn create() {}

pub fn start(_setup: &Setup) {}

pub fn environment() -> Vec<(String, String)> {
    Vec::new()
}

pub fn running() -> bool {
    false
}

pub fn before(_context: &Context, _rect: Rect, _document: &mut Document) {}

pub fn after(_context: &Context, _document: &mut Document) {}

pub fn replace_gpu(_setup: &Setup) {}

pub fn exiting() {}

pub fn set_owned(_owned: bool) {}

pub fn revision() -> u64 {
    0
}

pub fn listed(_hidden: &[HostWindowId]) -> Vec<HostWindow> {
    Vec::new()
}

pub fn opening_windows() -> Vec<(uuid::Uuid, HostWindowId)> {
    Vec::new()
}

pub fn close(_window: HostWindowId) {}

pub fn fullscreen(_window: HostWindowId, _fullscreen: bool) {}

pub fn focus(_window: HostWindowId) {}

pub fn show(_window: HostWindowId) {}

pub fn hide(_window: HostWindowId) {}

pub fn launch(_command: String) -> bool {
    false
}

pub fn exec(_arguments: Vec<String>, _launch: Option<u64>) -> bool {
    false
}

#[component]
pub fn WindowActions() -> NodeId {
    view! {
        <Frame />
    }
}

#[component]
pub fn WindowSurface(window: HostWindowId) -> NodeId {
    let _ = window;
    view! {
        <Frame />
    }
}

pub struct Opener {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Default)]
pub struct Programs;

impl Programs {
    pub fn scanned(&self) -> bool {
        false
    }

    pub fn openers(&self) -> Vec<Opener> {
        Vec::new()
    }

    pub fn open_file(
        &mut self,
        _key: &str,
        _file: &std::path::Path,
        _block: uuid::Uuid,
    ) -> Result<(), String> {
        Err("Programs cannot run here.".to_owned())
    }

    pub fn scan(&mut self, _pixels: u32) {}

    pub fn receive(&mut self) -> bool {
        false
    }

    pub fn listed(&self) -> Vec<block_plugin_api::HostProgram> {
        Vec::new()
    }

    pub fn launch(&mut self, _key: &str, _launch: Option<u64>) -> bool {
        false
    }
}
