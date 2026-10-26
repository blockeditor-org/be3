use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;

mod ui;

use ui::CheckoutView;

pub struct CheckoutApp;

#[component]
impl block_editor_beui::BeuiApp for CheckoutApp {
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
