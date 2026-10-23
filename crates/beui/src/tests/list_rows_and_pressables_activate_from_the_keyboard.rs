use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn list_rows_and_pressables_activate_from_the_keyboard() {
    let _root = crate::reactive::RootScope::new();
    let count = create_signal(0);
    let (document, [_row, _pressable]) = toolbar_of(|| {
        [
            view! {
                <ListRow on_click={move || count.update(|count| *count += 1)}>
                    <Text color=Color32::WHITE>"Row"</Text>
                </ListRow>
            },
            view! {
                <unstyled::Pressable on_click={move || count.update(|count| *count += 1)}>
                    <Text color=Color32::WHITE>"Press"</Text>
                </unstyled::Pressable>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(count.get_untracked(), 2);
}
