use be_block::{
    CheckoutContent, DisplaySettingsContent, HomeContent, InputRemapperContent,
    InputSettingsContent, OfflineSettingsContent, OrganizationPoliciesContent,
    PrefixRegistryContent, RepositoryContent, SettingsContent, SettingsProfileContent, TagContent,
    TagsContent, search_index::SearchIndexContent,
};

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
