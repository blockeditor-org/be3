use be_block::{
    CheckoutContent, DisplaySettingsContent, InputRemapperContent, InputSettingsContent,
    RepositoryContent, SettingsContent, SettingsProfileContent,
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
    ]
}
