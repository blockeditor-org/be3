use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::{Editor, LayeredSettings};
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;

mod appearance;
mod modes;
mod power;
mod screens;
mod ui;

use ui::DisplaySettingsView;

const INTRINSIC_WIDTH: f32 = 420.0;
const INTRINSIC_HEIGHT: f32 = 520.0;

pub struct DisplaySettingsApp;

impl block_editor_beui::BeuiApp for DisplaySettingsApp {
    fn view(editor: Editor) -> NodeId {
        let settings = LayeredSettings::<DisplaySettings>::page(editor);
        let content = editor.loaded::<block_editor_beui::be_block::DisplaySettingsContent>(&[]);
        let ready = settings.ready();
        let loaded = create_memo(move || content.get() && ready.get());
        view! {
            <Loaded loaded={loaded}>
                <DisplaySettingsView editor={editor} settings={settings} />
            </Loaded>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[cfg(test)]
mod tests;
