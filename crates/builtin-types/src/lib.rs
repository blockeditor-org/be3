use be_block::{
    CanvasContent, CounterContent, DatabaseContent, DatabaseSchemaContent, DatabaseViewContent,
    DeterministicGameContent, GameModuleContent, HotbarContent, PresentationContent, TextContent,
    VideoContent,
};
use be_block::{
    CheckoutContent, DisplaySettingsContent, EditorViewContent, InputRemapperContent,
    InputSettingsContent, RepositoryContent, SettingsContent, SettingsProfileContent,
};
use block_type_guest::{Registration, be_block_types};

be_block_types![
    TextContent { undo, file },
    CanvasContent { undo },
    CounterContent { undo },
    DatabaseContent { undo },
    DatabaseSchemaContent { undo },
    DatabaseViewContent { undo },
    DeterministicGameContent { undo },
    GameModuleContent,
    HotbarContent { undo },
    PresentationContent { undo },
    VideoContent { undo },
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
