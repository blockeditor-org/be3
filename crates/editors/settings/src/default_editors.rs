use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;

mod ui;

use ui::DefaultEditorsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;

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
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[cfg(test)]
mod tests;
