use be_sdk_beui::Editor;
use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;

mod ui;

use ui::CheckoutView;

pub struct CheckoutRoot;

#[component]
impl be_sdk_beui::BeuiRoot for CheckoutRoot {
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
