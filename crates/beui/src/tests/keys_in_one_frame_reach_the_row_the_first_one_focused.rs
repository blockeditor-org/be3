use super::*;
use crate::reactive::{NodeRef, view};
use crate::styled::MenuButton;

#[test]
fn keys_in_one_frame_reach_the_row_the_first_one_focused() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let (selected, set_selected) = create_signal(Vec::new());
    let (document, _) = toolbar_of({
        move || {
            let items = view! {
                <unstyled::MenuItem label="First" />
                <unstyled::MenuItem label="Second" />
            };
            [view! {
                <MenuButton
                    @node_ref=&button
                    label="More"
                    items
                    on_select={move |path: Vec<usize>| {
                        set_selected.update(|selected| selected.push(path));
                    }}
                />
            }]
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(button));
    harness.frame(Vec::new());

    harness.frame(vec![
        key_event(Key::ArrowDown, true, Modifiers::NONE),
        key_event(Key::ArrowDown, false, Modifiers::NONE),
        key_event(Key::Enter, true, Modifiers::NONE),
        key_event(Key::Enter, false, Modifiers::NONE),
    ]);
    harness.frame(Vec::new());

    assert_eq!(selected.get_untracked().as_slice(), &[vec![0usize]]);
}
