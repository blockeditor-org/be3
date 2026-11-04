use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_menu_opened_by_the_keyboard_starts_on_its_first_item() {
    let _root = crate::reactive::RootScope::new();
    let selected = create_signal(Vec::new());
    let document = build_node(move || {
        let items = view! {
            <unstyled::MenuItem label="Hide" />
            <unstyled::MenuItem label="Close" />
            <unstyled::MenuItem label="Quit" />
        };
        view! {
            <List spacing=0.0>
                <ContextMenu
                    items
                    on_select={move |path| selected.update(|selected| selected.push(path))}
                >
                    <Button @test_id={"held"} label="Window" />
                </ContextMenu>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("held")));
    harness.frame(Vec::new());

    harness.key(Key::F10, Modifiers::SHIFT);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        selected.get_untracked().as_slice(),
        &[vec![1usize]],
        "Shift+F10 opens on Hide, so Down then Enter chooses Close"
    );

    let pos = harness.center(harness.find("held"));
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: false,
    }]);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        selected.get_untracked().as_slice(),
        &[vec![1usize], vec![0usize]],
        "a menu the pointer opened, even after the keyboard was in use, starts on none of its items"
    );
}
