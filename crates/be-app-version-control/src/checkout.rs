use be_sdk_beui::Editor;
use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;

mod ui;

use ui::CheckoutView;

pub struct CheckoutApp;

#[component]
impl be_sdk_beui::BeuiApp for CheckoutApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <CheckoutView editor={editor} />
                </Loaded>
            </Root>
        }
    }
}
