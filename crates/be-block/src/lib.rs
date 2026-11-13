//! The host's built-in block types: settings and their kinds, profiles, organizations,
//! version control, the search index and the rest. The content core they are written against,
//! which type modules use alone, is be-block-content.

pub mod device_class;
pub mod display;
pub mod display_settings;
pub mod input_remapper;
pub mod input_settings;
pub mod organization;
pub mod profile;
pub mod resolution;
pub mod search_index;
pub mod settings;
pub mod version_control;
pub mod workspace_settings;

pub use be_block_graph::{ANYONE_INSIDE, WORKSPACE_TYPE};
pub use device_class::{ClassDefault, DeviceClass};
pub use display::quoted_name;
pub use display_settings::{DisplaySettings, DisplaySettingsContent, Motion, ThemeKind};
pub use input_remapper::{InputRemapper, InputRemapperContent};
pub use input_settings::{InputSettings, InputSettingsContent};
pub use organization::{
    Allowance, LostAccess, OfflineChoice, OfflinePolicy, OfflineSettings, OfflineSettingsContent,
    OrganizationPolicies, OrganizationPoliciesContent, SYSTEM_TYPES, UNFILED_TYPE, WhoMay,
    is_workspace_type, my_files, my_files_of, organization_settings, unfiled, unfiled_of,
};
pub use profile::{DESKTOP_EDITOR, FILES_EDITOR, RecentBlock, Recents, WORKSPACE_EDITOR};
pub use settings::{
    Resolution, ResolutionLayers, Settings, SettingsContent, SettingsProfile,
    SettingsProfileContent, SettingsSource, holds_owner_only,
};
pub use version_control::{
    Checkout, CheckoutConflict, CheckoutContent, ConflictKind, MAIN_BRANCH, Repository,
    RepositoryContent,
};
pub use workspace_settings::{
    EDIT_KINDS, Home, HomeContent, PREFERRED_EDITOR, PREFIXES, PREFIXES_TYPE, PrefixRegistry,
    PrefixRegistryContent, TAG_COLOR, TAG_DEFINITIONS, TAGGED, TAGS_TYPE, Tag, TagColor,
    TagContent, Tags, TagsContent, edit_kind_key, normal_prefix,
};

#[cfg(test)]
mod tests;
