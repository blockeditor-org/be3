use super::*;
use crate::reactive::{Action, Chord, NodeRef, build, create_signal, view};
use crate::styled::CommandPalette;
use crate::unstyled::command_palette_shown;

#[test]
fn a_global_action_runs_from_a_text_field_inside_an_open_palette() {
    let ran = Rc::new(RefCell::new(Vec::new()));
    let (lock, louder, typed) = (ran.clone(), ran.clone(), ran.clone());
    let palette = NodeRef::new();
    let held = palette.clone();
    let document = build(move || {
        Action::new("test.lock", "Lock screen", move || {
            lock.borrow_mut().push("lock")
        })
        .shortcut(Chord::logo(Key::L))
        .global()
        .register();
        Action::new("test.louder", "Volume up", move || {
            louder.borrow_mut().push("louder")
        })
        .shortcut(Chord::key(Key::VolumeUp))
        .global()
        .register();
        Action::new("test.typed", "Line tool", move || {
            typed.borrow_mut().push("typed")
        })
        .shortcut(Chord::key(Key::L))
        .global()
        .register();
        let (open, set_open) = create_signal(true);
        view! {
            <List spacing=0.0>
                <CommandPalette @node_ref=&held open on_close={move || set_open.set(false)} />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        command_palette_shown(harness.document(), palette.get()),
        vec!["test.lock", "test.louder", "test.typed"],
        "the palette lists global actions"
    );

    harness.key(Key::L, Modifiers::LOGO);
    harness.key(Key::VolumeUp, Modifiers::NONE);
    harness.key(Key::L, Modifiers::NONE);

    assert_eq!(
        *ran.borrow(),
        vec!["lock", "louder"],
        "Super chords and media keys reach global actions while a menu and a text field have the focus, and a plain letter stays the field's"
    );
    assert_eq!(Chord::logo(Key::L).shift().label(), "Super+Shift+L");
    assert_eq!(Chord::key(Key::VolumeUp).label(), "Volume Up");
}
