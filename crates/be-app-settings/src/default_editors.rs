use be_sdk_beui::Loaded;
use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{Editor, LayeredSettings};
use be_type_settings::EditorSettings;

mod ui;

use beui::ItemSize;
use ui::DefaultEditorsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;

pub struct DefaultEditorsRoot;

#[component]
impl be_sdk_beui::BeuiApp for DefaultEditorsRoot {
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
