use be_sdk_beui::Editor;
use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;

mod ui;

use ui::CompiledLogicView;

pub struct CompiledLogicRoot;

#[component]
impl be_sdk_beui::BeuiApp for CompiledLogicRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <CompiledLogicView editor={editor} />
                </Loaded>
            </Root>
        }
    }
}
