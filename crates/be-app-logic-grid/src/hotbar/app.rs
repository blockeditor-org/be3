use be_sdk_beui::Editor;
use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;

mod ui;

use ui::HotbarView;

pub struct HotbarApp;

#[component]
impl be_sdk_beui::BeuiApp for HotbarApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <HotbarView editor={editor} />
                </Loaded>
            </Root>
        }
    }
}

#[cfg(test)]
mod tests;
