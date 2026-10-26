use beui::KeyAt;
use block_editor_beui::beui::Key;
use block_editor_beui::beui::dock::unstyled::{DockingLayout, TabId};
use block_editor_beui::beui::icons::{ICON_BRING_TO_FRONT, ICON_CIRCLE_X};
use block_editor_beui::beui::reactive::*;

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
        Action::new(id, label, move || {
            match layout.switching() {
                true => layout.step_switch(backwards),
                false => layout.begin_switch(backwards),
            }
            if !modifiers.get_untracked().alt {
                layout.commit_switch();
            }
        })
        .glyph(ICON_BRING_TO_FRONT)
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
    .glyph(ICON_CIRCLE_X)
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
