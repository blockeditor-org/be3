use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_shell::RenameView;

pub struct RenameApp;

#[component]
impl block_editor_beui::BeuiApp for RenameApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let shown = match editor.host().renaming() {
            Some(block) => view! {
                <RenameView editor block />
            },
            None => view! {
                <Frame />
            },
        };
        view! {
            <Root clear_color=ColorRole::SurfaceRaised>{shown}</Root>
        }
    }
}
