use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn clicking_a_checkbox_toggles_it() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox
                label="Show timings"
                checked=false
                on_change={move |checked| {
                    set_changes.update(|changes| changes.push(checked));
                }}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(styled::checkbox_checked(harness.document(), checkbox));
    assert_eq!(changes.get_untracked(), [true]);

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(!styled::checkbox_checked(harness.document(), checkbox));
    assert_eq!(changes.get_untracked(), [true, false]);
}
