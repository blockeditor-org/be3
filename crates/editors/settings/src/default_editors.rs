use block_editor_beui::Loaded;
use block_editor_beui::be_block::EditorSettings;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Editor, LayeredSettings};

mod ui;

use ui::DefaultEditorsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;

pub struct DefaultEditorsApp;

impl block_editor_beui::BeuiApp for DefaultEditorsApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Loaded>|| {
                let settings = LayeredSettings::<EditorSettings>::page(editor);
                let device = settings.on_this_device();
                editor.block_types_known();
                view! {
                <List spacing=0.0>
                    <Keyed value={device} key={|device: bool| device}>|_device: ReadSignal<bool>|
                        <DefaultEditorsView
                            editor={editor}
                            settings={settings}
                            @sizing=ItemSize::Percent(100.0)
                        />
                    </Keyed>
                </List>
                }
            }</Loaded>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[cfg(test)]
mod tests;
