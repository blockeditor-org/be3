use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use be_type_deterministic_game::{GameModuleContent, GameModuleFile};
use tabletop_host::Game;

mod ui;

use ui::ModuleView;

const INTRINSIC_SIZE: Vec2 = Vec2::new(320.0, 120.0);

pub struct GameModuleRoot;

#[component]
impl be_sdk_beui::BeuiApp for GameModuleRoot {
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
