use be_sdk_beui::Editor;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_shell::RenameView;

pub struct RenameRoot;

#[component]
impl be_sdk_beui::BeuiRoot for RenameRoot {
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
