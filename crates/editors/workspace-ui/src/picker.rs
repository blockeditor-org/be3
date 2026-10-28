use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_shell::{Picker, PickerView};

pub struct PickerApp;

#[component]
impl block_editor_beui::BeuiApp for PickerApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let shown = match Picker::new(editor) {
            Some(picker) => view! {
                <PickerView picker />
            },
            None => view! {
                <Frame />
            },
        };
        view! {
            <Root clear_color=ColorRole::None>{shown}</Root>
        }
    }
}
