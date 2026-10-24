use be_block::{
    BeuiDemoContent, BrowserTabContent, CalendarContent, CanvasContent, CompiledLogicContent,
    CounterContent, DatabaseContent, DatabaseSchemaContent, DatabaseViewContent,
    DeterministicGameContent, FileContent, FolderContent, GameModuleContent,
    HotbarContent, LogicGameContent, LogicGridContent, MapContent, PanZoomContent, PixelArtContent,
    PixelRayTracerContent, PresentationContent, Scene3dContent, TerminalContent, TextContent,
    TriangleContent, VideoContent, WorkspaceContent,
};
use be_block::{
    CheckoutContent, DisplaySettingsContent, EditorViewContent, InputRemapperContent,
    InputSettingsContent, RepositoryContent, SettingsContent, SettingsProfileContent,
};
use block_type_guest::{Registration, be_block_types};

be_block_types![
    TextContent { undo, file },
    FileContent { file },
    BeuiDemoContent,
    BrowserTabContent,
    CalendarContent { undo },
    CanvasContent { undo },
    CompiledLogicContent,
    CounterContent { undo },
    DatabaseContent { undo },
    DatabaseSchemaContent { undo },
    DatabaseViewContent { undo },
    DeterministicGameContent { undo },
    FolderContent { directory },
    GameModuleContent,
    HotbarContent { undo },
    LogicGameContent { undo },
    LogicGridContent { undo },
    MapContent { undo },
    PanZoomContent,
    PixelArtContent { undo },
    PixelRayTracerContent { undo },
    PresentationContent { undo },
    Scene3dContent,
    TerminalContent,
    TriangleContent,
    VideoContent { undo },
    WorkspaceContent { root },
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
