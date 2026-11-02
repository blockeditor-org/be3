mod editor_settings;

pub use editor_settings::{EditorSettings, EditorSettingsContent};

be_sdk_type_guest::be_block_types![EditorSettingsContent];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);
