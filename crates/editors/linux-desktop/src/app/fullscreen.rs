use beui::KeyAt;
use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::ICON_MAXIMIZE;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Editor, HostWindows, WindowAction};

pub(super) fn bind_fullscreen(editor: Editor) {
    let windows = editor.host_value::<HostWindows>();
    Action::new("desktop.fullscreen", "Toggle fullscreen", move || {
        let focused = windows.with_untracked(|windows| {
            windows
                .iter()
                .find(|window| window.focused)
                .map(|window| (window.id, window.fullscreen.is_some()))
        });
        if let Some((window, fullscreen)) = focused {
            editor.act(WindowAction::Fullscreen {
                window,
                fullscreen: !fullscreen,
            });
        }
    })
    .glyph(ICON_MAXIMIZE)
    .shortcut(Chord::logo(Key::F))
    .at(KeyAt::Window)
    .capture()
    .register();
}
