use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use deterministic_game_types::{GameModuleContent, GameModuleFile};
use game_host::Game;

mod ui;

use ui::ModuleView;

const INTRINSIC_SIZE: Vec2 = Vec2::new(320.0, 120.0);

pub struct GameModuleApp;

#[component]
impl block_editor_beui::BeuiApp for GameModuleApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <ModuleView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Creation(creation: Creation) -> NodeId {
        view! {
            <ContentFileCreation
                creation
                title="New Game Module"
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
        GameModuleFile::FILE_EXTENSIONS,
        GameModuleFile::MIME_TYPES,
    )
}

pub(crate) fn imported(file: PickedFile) -> Result<GameModuleContent, String> {
    let PickedFile { name, data } = file;
    Game::load(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(GameModuleFile::from_file(name, data))
}
