use accesskit::Role;

use super::*;
use crate::reactive::view;
use crate::styled::{ButtonVariant, SplitButton};
use crate::unstyled::MenuItem;

#[test]
fn a_split_button_runs_its_action_or_opens_its_menu() {
    let clicks = Rc::new(Cell::new(0));
    let picked = Rc::new(RefCell::new(Vec::new()));
    let (clicked, chosen) = (clicks.clone(), picked.clone());
    let (document, [split]) = toolbar_of(|| {
        [view! {
            <SplitButton
                label="Run"
                variant=ButtonVariant::Primary
                menu_label="More ways to run it"
                items={view! {
                    <MenuItem label="Run with fresh data" />
                    <MenuItem label="Install its launcher" />
                }}
                on_click={move || clicked.set(clicked.get() + 1)}
                on_select={move |path: Vec<usize>| chosen.borrow_mut().push(path)}
            />
        }]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let buttons: Vec<Option<String>> = harness
        .accessible()
        .iter()
        .filter(|node| node.role() == Role::Button)
        .map(|node| node.label().map(str::to_owned))
        .collect();
    assert_eq!(
        buttons,
        [
            Some("Run".to_owned()),
            Some("More ways to run it".to_owned())
        ]
    );

    let rect = harness.rect(split);
    let first = harness
        .document()
        .focusables_within(split)
        .first()
        .copied()
        .expect("the split button can take the focus");
    harness.click(harness.center(first));
    assert_eq!(clicks.get(), 1);
    assert!(picked.borrow().is_empty());

    harness.click(pos2(
        rect.left() + harness.rect(first).width() + 12.0,
        rect.center().y,
    ));
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        clicks.get(),
        1,
        "the arrow opens the menu without running the action"
    );
    assert_eq!(*picked.borrow(), [vec![1]]);
}
