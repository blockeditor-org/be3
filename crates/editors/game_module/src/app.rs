use block_client::blocks::game_module::GameModule;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor, FileFilter, PickedFile};
use game_host::Game;

mod ui;

use ui::{ModuleCreation, ModuleView};

const INTRINSIC_SIZE: Vec2 = Vec2::new(320.0, 120.0);

pub struct GameModuleApp;

impl block_editor_plugin::BeuiApp for GameModuleApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <ModuleView editor={editor} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        view! {
            <ModuleCreation creation={creation} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

pub(crate) fn filter() -> FileFilter {
    FileFilter {
        name: "Game modules".to_owned(),
        default_file_name: "Game".to_owned(),
        extensions: GameModule::FILE_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: GameModule::MIME_TYPES
            .iter()
            .map(|mime_type| (*mime_type).to_owned())
            .collect(),
    }
}

pub(crate) fn imported(file: PickedFile) -> Result<GameModule, String> {
    let PickedFile { name, data } = file;
    Game::load(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(GameModule::new(name, data))
}
