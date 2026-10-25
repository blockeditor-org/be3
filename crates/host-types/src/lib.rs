use be_block::{
    CheckoutContent, DisplaySettingsContent, HomeContent, InputRemapperContent,
    InputSettingsContent, OfflineSettingsContent, OrganizationPoliciesContent,
    PrefixRegistryContent, RepositoryContent, SettingsContent, SettingsProfileContent, TagContent,
    TagsContent,
};
use block_type_guest::Registration;

pub fn registrations() -> Vec<Registration> {
    vec![
        Registration::of::<DisplaySettingsContent>(),
        Registration::of::<InputSettingsContent>(),
        Registration::of::<InputRemapperContent>(),
        Registration::of::<SettingsContent>().undo::<SettingsContent>(),
        Registration::of::<SettingsProfileContent>().undo::<SettingsProfileContent>(),
        Registration::of::<RepositoryContent>(),
        Registration::of::<CheckoutContent>(),
        Registration::of::<PrefixRegistryContent>(),
        Registration::of::<TagsContent>().undo::<TagsContent>(),
        Registration::of::<TagContent>().undo::<TagContent>(),
        Registration::of::<HomeContent>().undo::<HomeContent>(),
        Registration::of::<OrganizationPoliciesContent>(),
        Registration::of::<OfflineSettingsContent>(),
    ]
}
