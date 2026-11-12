use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn keys_in_one_frame_reach_the_row_the_first_one_focused() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let selected = create_signal(Vec::new());
    let (document, _) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="First" on_click={move || selected.update(|selected| selected.push(vec![0]))} />
            <unstyled::MenuItem label="Second" on_click={move || selected.update(|selected| selected.push(vec![1]))} />
        };
        [view! {
            <MenuButton
                @node_ref=&button
                label="More"
                items
            />
        }]
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(button));
    harness.frame(Vec::new());

    harness.frame(vec![
        key_event(Key::ArrowDown, true),
        key_event(Key::ArrowDown, false),
        key_event(Key::Enter, true),
        key_event(Key::Enter, false),
    ]);
    harness.frame(Vec::new());

    assert_eq!(selected.get_untracked().as_slice(), &[vec![0usize]]);
}
