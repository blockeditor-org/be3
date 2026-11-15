use be_block::{
    DisplaySettingsContent, HomeContent, OfflineSettingsContent, OrganizationPoliciesContent,
    PrefixRegistryContent, SettingsContent, SettingsProfileContent, TagContent, TagsContent,
};
use be_block_input::{InputRemapperContent, InputSettingsContent};
use be_block_repository::{CheckoutContent, RepositoryContent};
use be_block_search_index::SearchIndexContent;

be_sdk_type_guest::be_block_types![
    DisplaySettingsContent,
    InputSettingsContent,
    InputRemapperContent,
    SettingsContent { undo },
    SettingsProfileContent { undo },
    RepositoryContent,
    CheckoutContent,
    PrefixRegistryContent,
    TagsContent { undo },
    TagContent { undo },
    HomeContent { undo },
    OrganizationPoliciesContent,
    OfflineSettingsContent,
    SearchIndexContent { merge_parts },
];
