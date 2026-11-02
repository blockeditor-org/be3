use be_sdk_beui::Loaded;
use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{Editor, LayeredSettings};

mod appearance;
pub(crate) mod modes;
mod power;
mod screens;
mod ui;

use beui::ItemSize;
use ui::DisplaySettingsView;

const INTRINSIC_WIDTH: f32 = 420.0;
const INTRINSIC_HEIGHT: f32 = 520.0;

pub struct DisplaySettingsRoot;

#[component]
impl be_sdk_beui::BeuiApp for DisplaySettingsRoot {
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
