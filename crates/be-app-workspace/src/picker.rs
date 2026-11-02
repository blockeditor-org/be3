use be_sdk_beui::Editor;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_shell::{Picker, PickerView};

pub struct PickerApp;

#[component]
impl be_sdk_beui::BeuiApp for PickerApp {
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
