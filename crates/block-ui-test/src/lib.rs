mod beui;
mod document;
mod link;
mod overflow;

mod snapshot;
mod store;
#[cfg(test)]
mod tests;

pub use beui::{BeuiTest, HostCall};
pub use block_plugin_api::{PickedBlock, ops::Reply, requested};
pub use document::DocumentTest;
pub use snapshot::{Recording, Snapshots};
pub use store::ContentStore;

pub mod prelude {
    pub use crate::Snapshots;
    pub use beui::native::testing::Gui;
}
