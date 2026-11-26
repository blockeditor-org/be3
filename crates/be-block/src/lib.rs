//! The host's built-in block types every editor's frame reads: settings and their kinds,
//! profiles, display settings, organizations, workspaces and tags. The content core they are
//! written against, which type modules use alone, is be-block-content; the built-ins only some
//! editors read are crates of their own (be-block-input, be-block-repository,
//! be-block-search-index).

pub mod account_block;
pub mod device_class;
pub mod display_settings;
pub mod organization;
pub mod profile;
pub mod resolution;
pub mod root_settings;
pub mod settings;
pub mod workspace_settings;

pub use account_block::{AccountBlock, AccountBlockContent, account_block, account_block_of};
pub use be_block_graph::{ANYONE_INSIDE, WORKSPACE_TYPE};
pub use device_class::{ClassDefault, DeviceClass};
pub use display_settings::{DisplaySettings, DisplaySettingsContent, Motion, ThemeKind};
pub use organization::{
    Allowance, LinkSharing, LostAccess, OfflineChoice, OfflinePolicy, OfflineSettings,
    OfflineSettingsContent, OrganizationPolicies, OrganizationPoliciesContent, SYSTEM_TYPES,
    UNFILED_TYPE, WhoMay, is_workspace_type, my_files, my_files_of, organization_settings, unfiled,
    unfiled_of,
};
pub use profile::{DESKTOP_EDITOR, FILES_EDITOR, RecentBlock, Recents, WORKSPACE_EDITOR};
pub use settings::{
    Resolution, ResolutionLayers, Settings, SettingsContent, SettingsProfile,
    SettingsProfileContent, SettingsSource, holds_owner_only,
};
pub use workspace_settings::{
    EDIT_KINDS, Home, HomeContent, PREFERRED_EDITOR, PREFIXES, PREFIXES_TYPE, PrefixRegistry,
    PrefixRegistryContent, TAG_COLOR, TAG_DEFINITIONS, TAGGED, TAGS_TYPE, Tag, TagColor,
    TagContent, Tags, TagsContent, edit_kind_key, normal_prefix,
};

#[cfg(test)]
mod tests;
