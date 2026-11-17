extern crate be_sdk_beui_platform as _;

mod beui;
mod document;
mod fetches;
mod link;
mod signatures;
mod store;
#[cfg(test)]
mod tests;
mod workers;

pub use be_sdk_api::{
    PickedBlock,
    ops::{Reply, wire},
    requested,
};
pub use beui::{AskedPreview, BeuiTest, HostCall};
pub use document::DocumentTest;
pub use fetches::Fetch;
pub use signatures::signature;
pub use store::ContentStore;

/// What a test drives and reads its program through (`Gui`) and paints it with
/// (`Snapshots`), for both `BeuiTest` and `DocumentTest`.
pub mod prelude {
    pub use beui_host::native::testing::{Gui, Snapshots, Target};
}
