use be_block::{
    CheckoutContent, DisplaySettingsContent, EditorViewContent, InputRemapperContent,
    InputSettingsContent, RepositoryContent, SettingsContent, SettingsProfileContent,
};
use be_block::{
    CounterContent, 
};
use block_type_guest::{Registration, be_block_types};

be_block_types![
    CounterContent { undo },
];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);

pub fn host() -> Vec<Registration> {
    vec![
        Registration::of::<DisplaySettingsContent>(),
        Registration::of::<InputSettingsContent>(),
        Registration::of::<InputRemapperContent>(),
        Registration::of::<SettingsContent>().undo::<SettingsContent>(),
        Registration::of::<SettingsProfileContent>().undo::<SettingsProfileContent>(),
        Registration::of::<EditorViewContent>(),
        Registration::of::<RepositoryContent>(),
        Registration::of::<CheckoutContent>(),
    ]
}

#[cfg(test)]
mod tests;
