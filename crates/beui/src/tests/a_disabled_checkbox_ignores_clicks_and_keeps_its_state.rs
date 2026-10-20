use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_disabled_checkbox_ignores_clicks_and_keeps_its_state() {
    let _root = crate::reactive::RootScope::new();
    let checked = create_signal(false);
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox label="Minimum" checked={checked} disabled=true />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(!styled::checkbox_checked(harness.document(), checkbox));
    assert!(!checked.get_untracked());
}
