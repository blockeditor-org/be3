use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn a_disabled_checkbox_ignores_clicks_and_keeps_its_state() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <Checkbox
                label="Minimum"
                checked=false
                disabled=true
                on_change={move |checked| sink.borrow_mut().push(checked)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(!styled::checkbox_checked(harness.document(), checkbox));
    assert!(changes.borrow().is_empty());
}
