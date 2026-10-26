use super::*;
use crate::KeyAt;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::CommandPaletteApi;

#[test]
fn a_global_action_runs_from_a_text_field_inside_an_open_palette() {
    let _root = crate::reactive::RootScope::new();
    let ran = create_signal(Vec::new());
    let palette = NodeRef::new();
    let document = build_node(move || {
        Action::new("test.lock", "Lock screen", move || {
            ran.update(|ran| ran.push("lock"))
        })
        .shortcut(Chord::logo(Key::L))
        .at(KeyAt::Window)
        .register();
        Action::new("test.louder", "Volume up", move || {
            ran.update(|ran| ran.push("louder"))
        })
        .shortcut(Chord::key(Key::VolumeUp))
        .at(KeyAt::Window)
        .register();
        Action::new("test.typed", "Line tool", move || {
            ran.update(|ran| ran.push("typed"))
        })
        .shortcut(Chord::key(Key::L))
        .at(KeyAt::Window)
        .register();
        let open = create_signal(true);
        view! {
            <List spacing=0.0>
                <CommandPalette @node_ref=&palette open />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<CommandPaletteApi>(palette.get())
            .shown(),
        vec!["test.lock", "test.louder", "test.typed"],
        "the palette lists global actions"
    );

    harness.key(Key::L, Modifiers::LOGO);
    harness.key(Key::VolumeUp, Modifiers::NONE);
    harness.key(Key::L, Modifiers::NONE);

    assert_eq!(
        ran.get_untracked(),
        vec!["lock", "louder"],
        "Super chords and media keys reach global actions while a menu and a text field have the focus, and a plain letter stays the field's"
    );
    assert_eq!(Chord::logo(Key::L).shift().label(), "Super+Shift+L");
    assert_eq!(Chord::key(Key::VolumeUp).label(), "Volume Up");
}
