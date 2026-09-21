use block::Block;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{
    close, content, flush, installed_for, open, operate, start, status, stop, workspace,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{close, content, flush, open, operate, status, stop};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct Config {
    pub(crate) url: String,
    pub(crate) directory: std::path::PathBuf,
    pub(crate) account: Uuid,
    pub(crate) workspace: Uuid,
    pub(crate) email: String,
    pub(crate) display_name: String,
    pub(crate) password: String,
    pub(crate) key: [u8; 32],
    pub(crate) be_workspace: Option<Uuid>,
    pub(crate) context: eframe::egui::Context,
}

#[derive(Clone)]
pub(crate) struct Content {
    pub(crate) content_type: Uuid,
    pub(crate) bytes: Vec<u8>,
    pub(crate) revision: u64,
}

#[derive(Default)]
pub(crate) struct Status {
    pub(crate) running: bool,
    pub(crate) connected: bool,
    pub(crate) blocks: usize,
    pub(crate) unsealed: usize,
    pub(crate) error: Option<String>,
}

pub(crate) fn content_type_for(block_type: Uuid) -> Option<Uuid> {
    (block_type == block_client::blocks::counter::Counter::TYPE_ID)
        .then_some(<be_block::CounterContent as be_block::BlockContent>::CONTENT_TYPE)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
