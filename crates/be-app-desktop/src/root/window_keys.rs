use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Key, NodeId};
use be_sdk_beui::{Editor, SuperKeptWatch};
use be_sdk_shell::{WindowActions, WindowManager};
use beui::KeyAt;

use super::fullscreen::fullscreen_action;

#[derive(Clone, Copy)]
pub(crate) struct WindowKeys {
    pub(crate) close: Chord,
    pub(crate) snap_left: Chord,
    pub(crate) snap_right: Chord,
    pub(crate) maximize: Chord,
    pub(crate) down: Chord,
    pub(crate) show_desktop: Chord,
    pub(crate) next_display: Chord,
    pub(crate) previous_display: Chord,
    pub(crate) next_of_app: Chord,
    pub(crate) fullscreen: Chord,
}

impl WindowKeys {
    pub(crate) fn of(super_kept: bool) -> Self {
        match super_kept {
            false => Self {
                close: Chord::key(Key::F4).alt(),
                snap_left: Chord::logo(Key::ArrowLeft),
                snap_right: Chord::logo(Key::ArrowRight),
                maximize: Chord::logo(Key::ArrowUp),
                down: Chord::logo(Key::ArrowDown),
                show_desktop: Chord::logo(Key::D),
                next_display: Chord::logo(Key::ArrowRight).shift(),
                previous_display: Chord::logo(Key::ArrowLeft).shift(),
                next_of_app: Chord::logo(Key::Backtick),
                fullscreen: Chord::logo(Key::F),
            },
            true => Self {
                close: Chord::ctrl(Key::F4),
                snap_left: Chord::ctrl(Key::ArrowLeft).alt(),
                snap_right: Chord::ctrl(Key::ArrowRight).alt(),
                maximize: Chord::ctrl(Key::ArrowUp).alt(),
                down: Chord::ctrl(Key::ArrowDown).alt(),
                show_desktop: Chord::ctrl(Key::D).alt(),
                next_display: Chord::ctrl(Key::ArrowRight).alt().shift(),
                previous_display: Chord::ctrl(Key::ArrowLeft).alt().shift(),
                next_of_app: Chord::ctrl(Key::Backtick).alt(),
                fullscreen: Chord::ctrl(Key::F).alt(),
            },
        }
    }
}

#[component]
pub(super) fn WindowShortcuts(editor: Editor, manager: WindowManager) -> NodeId {
    let super_kept = editor.watch::<SuperKeptWatch>();
    view! {
        <List spacing=Space::None>
            <Dynamic value={super_kept}>|super_kept: bool| {
                bind(editor, manager, WindowKeys::of(super_kept));
                view! {
                    <Frame />
                }
            }</Dynamic>
        </List>
    }
}

fn bind(editor: Editor, manager: WindowManager, keys: WindowKeys) {
    let WindowActions {
        close,
        snap_left,
        snap_right,
        maximize,
        restore,
        hide,
        show_desktop,
        next_display,
        previous_display,
        next_of_app,
    } = manager.actions();
    for (action, chord) in [
        (close, keys.close),
        (snap_left, keys.snap_left),
        (snap_right, keys.snap_right),
        (maximize, keys.maximize),
        (restore, keys.down),
        (hide, keys.down),
        (show_desktop, keys.show_desktop),
        (next_display, keys.next_display),
        (previous_display, keys.previous_display),
        (next_of_app, keys.next_of_app),
        (fullscreen_action(editor), keys.fullscreen),
    ] {
        action
            .shortcut(chord)
            .at(KeyAt::Window)
            .capture()
            .register();
    }
}
