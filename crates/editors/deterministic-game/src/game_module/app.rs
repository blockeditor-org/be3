use block_editor_beui::be_block::GameModuleContent;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use game_host::Game;

mod ui;

use ui::ModuleView;

const INTRINSIC_SIZE: Vec2 = Vec2::new(320.0, 120.0);

pub struct GameModuleApp;

impl block_editor_beui::BeuiApp for GameModuleApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <ModuleView editor={editor} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        view! {
            <ContentFileCreation
                creation
                id_prefix="game-module"
                filter={filter()}
                import={imported}
            />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

pub(crate) fn filter() -> FileFilter {
    FileFilter::new(
        "Game modules",
        GameModuleContent::FILE_EXTENSIONS,
        GameModuleContent::MIME_TYPES,
    )
}

pub(crate) fn imported(file: PickedFile) -> Result<GameModuleContent, String> {
    let PickedFile { name, data } = file;
    Game::load(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(GameModuleContent::from_file(name, data))
}
