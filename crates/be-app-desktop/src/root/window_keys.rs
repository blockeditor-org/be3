use be_sdk_beui::beui::dock::styled::{DockWindowActions, DockingLayout, TabId};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Key, NodeId};
use be_sdk_beui::{Editor, ScreenLockedWatch, SuperKeptWatch};
use beui::KeyAt;

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
pub(super) fn WindowShortcuts(editor: Editor, layout: DockingLayout<TabId>) -> NodeId {
    let super_kept = editor.watch::<SuperKeptWatch>();
    let locked = editor.watch::<ScreenLockedWatch>();
    view! {
        <List spacing=Space::None>
            <Dynamic value={super_kept}>|super_kept: bool| {
                bind(layout.window_actions(locked), WindowKeys::of(super_kept));
                view! {
                    <Frame />
                }
            }</Dynamic>
        </List>
    }
}

fn bind(actions: DockWindowActions, keys: WindowKeys) {
    let DockWindowActions {
        close,
        hide,
        snap_left,
        snap_right,
        maximize,
        restore,
        fullscreen,
        show_desktop,
        next_area,
        previous_area,
        next_of_group,
        switch_next,
        switch_previous,
        switch_cancel,
    } = actions;
    let switch = Chord::key(Key::Tab).alt();
    let escape = Chord::key(Key::Escape).alt();
    for (action, chords) in [
        (close, vec![keys.close]),
        (snap_left, vec![keys.snap_left]),
        (snap_right, vec![keys.snap_right]),
        (maximize, vec![keys.maximize]),
        (restore, vec![keys.down]),
        (hide, vec![keys.down]),
        (show_desktop, vec![keys.show_desktop]),
        (next_area, vec![keys.next_display]),
        (previous_area, vec![keys.previous_display]),
        (next_of_group, vec![keys.next_of_app]),
        (fullscreen, vec![keys.fullscreen]),
        (switch_next, vec![switch]),
        (switch_previous, vec![switch.shift()]),
        (switch_cancel, vec![escape, escape.shift()]),
    ] {
        let mut action = action;
        for chord in chords {
            action = action.shortcut(chord);
        }
        action.at(KeyAt::Window).capture().register();
    }
}
