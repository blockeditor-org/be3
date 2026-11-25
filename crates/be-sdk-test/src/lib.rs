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

/// A manifest of one editor, `test`, with no camera limits of its own: what a root a test
/// defines for itself, not one an app's `beui_app!` names, is an editor of (`test_editor!`).
pub const TEST_MANIFEST: &str = r#"{
    "id": "be3.test",
    "name": "Test",
    "version": "0.1.0",
    "editors": {
        "test": {
            "id": "00000000-0000-4000-8000-000000000001",
            "display_name": "Test",
            "icon": "\ue110",
            "views": ["Main"]
        }
    },
    "entry_point": "test.wasm"
}"#;

/// Makes a root a test defines for itself the editor `test` of `TEST_MANIFEST`, as `beui_app!`
/// does an app's roots, so `BeuiTest` can run it.
#[macro_export]
macro_rules! test_editor {
    ($root:ty) => {
        impl ::be_sdk_beui::AppEditor for $root {
            const MANIFEST: &'static str = $crate::TEST_MANIFEST;
            const EDITORS: &'static [&'static str] = &["test"];
        }
    };
}
pub use document::DocumentTest;
pub use fetches::Fetch;
pub use signatures::signature;
pub use store::ContentStore;

/// What a test drives and reads its program through (`Gui`) and paints it with
/// (`Snapshots`), for both `BeuiTest` and `DocumentTest`.
pub mod prelude {
    pub use beui_host::native::testing::{Gui, Match, Snapshots, Target, find_text};
}
