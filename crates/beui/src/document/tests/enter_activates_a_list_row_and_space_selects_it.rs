use super::*;
use crate::reactive::{Text, view};
use crate::styled::ListRow;

#[test]
fn enter_activates_a_list_row_and_space_selects_it() {
    let clicks = Rc::new(Cell::new(0));
    let activations = Rc::new(Cell::new(0));
    let (clicked, activated) = (clicks.clone(), activations.clone());
    let (document, [_row]) = toolbar_of(|| {
        [view! {
            <ListRow
                on_click={move || clicked.set(clicked.get() + 1)}
                on_activate={move || activated.set(activated.get() + 1)}
            >
                <Text string="Row" font_size=14.0 color=Color32::WHITE />
            </ListRow>
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(clicks.get(), 1);
    assert_eq!(activations.get(), 1);
}
