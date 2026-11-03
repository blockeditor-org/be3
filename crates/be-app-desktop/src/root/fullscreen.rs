use be_sdk_beui::beui::icons::ICON_MAXIMIZE;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::desktop;
use be_sdk_beui::{Editor, WindowAction, WindowsWatch};

pub(super) fn fullscreen_action(editor: Editor) -> ActionBuilder {
    let windows = editor.watch::<WindowsWatch>();
    Action::new("desktop.fullscreen", "Toggle fullscreen", move || {
        let focused = windows.with_untracked(|windows| {
            windows
                .iter()
                .find(|window| window.focused && window.shown)
                .map(|window| (window.id, window.fullscreen.is_some()))
        });
        if let Some((window, fullscreen)) = focused {
            editor.act(move || {
                desktop::windows::request(WindowAction::Fullscreen {
                    window,
                    fullscreen: !fullscreen,
                })
                .detach()
            });
        }
    })
    .glyph(ICON_MAXIMIZE)
}
