use block_editor_beui::Loaded;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Editor, LayeredSettings};
use settings_types::EditorSettings;

mod ui;

use ui::DefaultEditorsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;

pub struct DefaultEditorsApp;

#[component]
impl block_editor_beui::BeuiApp for DefaultEditorsApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>|| {
                    let settings = LayeredSettings::<EditorSettings>::page(editor);
                    let device = settings.on_this_device();
                    editor.block_types_known();
                    view! {
                        <List spacing=Space::None>
                            <Keyed
                                value={device}
                                key={|device: bool| device}
                            >|_device: ReadSignal<bool>|
                                <DefaultEditorsView
                                    editor={editor}
                                    settings={settings}
                                    @sizing=ItemSize::Percent(100.0)
                                />
                            </Keyed>
                        </List>
                    }
                }</Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[cfg(test)]
mod tests;
