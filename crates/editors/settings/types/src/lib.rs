mod editor_settings;

pub use editor_settings::{EditorSettings, EditorSettingsContent};

block_type_guest::be_block_types![EditorSettingsContent];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);
