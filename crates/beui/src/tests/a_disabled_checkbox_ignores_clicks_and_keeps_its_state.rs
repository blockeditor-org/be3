use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn a_disabled_checkbox_ignores_clicks_and_keeps_its_state() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox
                label="Minimum"
                checked=false
                disabled=true
                on_change={move |checked| set_changes.update(|changes| changes.push(checked))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(!styled::checkbox_checked(harness.document(), checkbox));
    assert!(changes.with_untracked(|changes| changes.is_empty()));
}
