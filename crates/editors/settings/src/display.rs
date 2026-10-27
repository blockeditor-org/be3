use block_editor_beui::Loaded;
use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Editor, LayeredSettings};

mod appearance;
pub(crate) mod modes;
mod power;
mod screens;
mod ui;

use beui::ItemSize;
use ui::DisplaySettingsView;

const INTRINSIC_WIDTH: f32 = 420.0;
const INTRINSIC_HEIGHT: f32 = 520.0;

pub struct DisplaySettingsApp;

#[component]
impl block_editor_beui::BeuiApp for DisplaySettingsApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>|| {
                    let settings = LayeredSettings::<DisplaySettings>::page(editor);
                    let device = settings.on_this_device();
                    view! {
                        <List spacing=Space::None>
                            <Keyed
                                value={device}
                                key={|device: bool| device}
                            >|_device: ReadSignal<bool>|
                                <DisplaySettingsView
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
