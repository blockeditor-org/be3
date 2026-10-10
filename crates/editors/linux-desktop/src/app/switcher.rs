use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::{ICON_CANCEL, ICON_FLIP_TO_FRONT};
use block_editor_beui::beui::reactive::{
    Action, Chord, clone, create_effect, create_memo, held_modifiers, untrack,
};
use block_editor_beui::beui::unstyled::{DockingLayout, TabId};

pub(super) fn bind_window_switcher(layout: &DockingLayout<TabId>) {
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
        let switching = layout.clone();
        let held = modifiers.clone();
        Action::new(id, label, move || {
            match switching.switching() {
                true => switching.step_switch(backwards),
                false => switching.begin_switch(backwards),
            }
            if !held.get_untracked().alt {
                switching.commit_switch();
            }
        })
        .glyph(ICON_FLIP_TO_FRONT)
        .shortcut(chord)
        .intercepts()
        .register();
    }
    let open = create_memo(clone!(layout -> move || layout.switching()));
    let cancelling = layout.clone();
    let escape = Chord::key(Key::Escape).alt();
    Action::new(
        "desktop.switch.cancel",
        "Stop switching windows",
        move || {
            cancelling.cancel_switch();
        },
    )
    .glyph(ICON_CANCEL)
    .shortcut(escape)
    .shortcut(escape.shift())
    .enabled(open)
    .intercepts()
    .register();
    let committing = layout.clone();
    create_effect(move || {
        if !modifiers.get().alt && untrack(|| committing.switching()) {
            untrack(|| committing.commit_switch());
        }
    });
}
