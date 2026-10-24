use be_block::{
    BrowserTabContent, CanvasContent, CompiledLogicContent, CounterContent, DatabaseContent,
    DatabaseSchemaContent, DatabaseViewContent, DeterministicGameContent, GameModuleContent,
    HotbarContent, LogicGameContent, LogicGridContent, MapContent, PixelArtContent,
    PresentationContent, TerminalContent, TextContent, VideoContent,
};
use be_block::{
    CheckoutContent, DisplaySettingsContent, EditorViewContent, InputRemapperContent,
    InputSettingsContent, RepositoryContent, SettingsContent, SettingsProfileContent,
};
use block_type_guest::{Registration, be_block_types};

be_block_types![
    TextContent { undo, file },
    BrowserTabContent,
    CanvasContent { undo },
    CompiledLogicContent,
    CounterContent { undo },
    DatabaseContent { undo },
    DatabaseSchemaContent { undo },
    DatabaseViewContent { undo },
    DeterministicGameContent { undo },
    GameModuleContent,
    HotbarContent { undo },
    LogicGameContent { undo },
    LogicGridContent { undo },
    MapContent { undo },
    PixelArtContent { undo },
    PresentationContent { undo },
    TerminalContent,
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
