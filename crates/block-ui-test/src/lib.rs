extern crate beui_platform_plugin as _;

mod beui;
mod document;
mod link;
mod overflow;

mod snapshot;
mod store;
#[cfg(test)]
mod tests;

pub use beui::{BeuiTest, HostCall};
pub use block_plugin_api::{
    PickedBlock,
    ops::{Reply, wire},
    requested,
};
pub use document::DocumentTest;
pub use store::ContentStore;
