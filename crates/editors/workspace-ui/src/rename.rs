use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_shell::RenameView;

pub struct RenameApp;

#[component]
impl block_editor_beui::BeuiApp for RenameApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let renaming = editor.host().renaming();
        let clear_color = match renaming.is_some_and(|rename| rename.sheet) {
            true => ColorRole::None,
            false => ColorRole::SurfaceRaised,
        };
        let shown = match renaming {
            Some(rename) => view! {
                <RenameView editor block={rename.block} sheet={rename.sheet} />
            },
            None => view! {
                <Frame />
            },
        };
        view! {
            <Root clear_color>{shown}</Root>
        }
    }
}
