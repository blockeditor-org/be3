use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;

mod editors;
mod ui;

use editors::DefaultEditorsView;
use ui::UiSettingsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 220.0;
const EDITORS_HEIGHT: f32 = 360.0;

pub struct UiSettingsApp;

impl block_editor_beui::BeuiApp for UiSettingsApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::UiSettingsContent>(&[]);
        view! {
            <Loaded loaded={loaded}>
                <UiSettingsView editor={editor} />
            </Loaded>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

pub struct DefaultEditorsApp;

impl block_editor_beui::BeuiApp for DefaultEditorsApp {
    fn view(editor: Editor) -> NodeId {
        let content = editor.loaded::<block_editor_beui::be_block::EditorSettingsContent>(&[]);
        let catalog = editor.block_types_known();
        let loaded = create_memo(move || content.get() && catalog.get());
        view! {
            <Loaded loaded={loaded}>
                <DefaultEditorsView editor={editor} />
            </Loaded>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, EDITORS_HEIGHT))
    }
}

#[cfg(test)]
mod tests;
