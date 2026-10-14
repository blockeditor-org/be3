use beui::KeyAt;
use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::{ICON_CANCEL, ICON_FLIP_TO_FRONT};
use block_editor_beui::beui::reactive::{
    Action, Chord, create_effect, create_memo, held_modifiers, untrack,
};
use block_editor_beui::beui::unstyled::{DockingLayout, TabId};

pub(super) fn bind_window_switcher(layout: DockingLayout<TabId>) {
    let modifiers = held_modifiers();
    let next = Chord::key(Key::Tab).alt();
    for (id, label, backwards, chord) in [
        (
            "desktop.switch.next",
            "Switch to the next window",
            false,
            next,
        ),
        (
            "desktop.switch.previous",
            "Switch to the previous window",
            true,
            next.shift(),
        ),
    ] {
        let held = modifiers;
        Action::new(id, label, move || {
            match layout.switching() {
                true => layout.step_switch(backwards),
                false => layout.begin_switch(backwards),
            }
            if !held.get_untracked().alt {
                layout.commit_switch();
            }
        })
        .glyph(ICON_FLIP_TO_FRONT)
        .shortcut(chord)
        .at(KeyAt::Window)
        .capture()
        .register();
    }
    let open = create_memo(move || layout.switching());
    let escape = Chord::key(Key::Escape).alt();
    Action::new(
        "desktop.switch.cancel",
        "Stop switching windows",
        move || {
            layout.cancel_switch();
        },
    )
    .glyph(ICON_CANCEL)
    .shortcut(escape)
    .shortcut(escape.shift())
    .enabled(open)
    .at(KeyAt::Window)
    .capture()
    .register();
    create_effect(move || {
        if !modifiers.get().alt && untrack(|| layout.switching()) {
            untrack(|| layout.commit_switch());
        }
    });
}
