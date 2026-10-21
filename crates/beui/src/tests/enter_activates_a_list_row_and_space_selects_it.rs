use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn enter_activates_a_list_row_and_space_selects_it() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let activations = create_signal(0);
    let (document, [_row]) = toolbar_of(|| {
        [view! {
            <ListRow
                on_click={move || clicks.update(|clicks| *clicks += 1)}
                on_activate={move || activations.update(|activations| *activations += 1)}
            >
                <Text color=Color32::WHITE>"Row"</Text>
            </ListRow>
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(clicks.get_untracked(), 1);
    assert_eq!(activations.get_untracked(), 1);
}
