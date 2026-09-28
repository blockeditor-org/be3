use super::*;
use crate::reactive::{build, create_signal, view};
use crate::styled::Select;
use crate::unstyled::ChoiceOption;

#[test]
fn a_select_following_its_prop_does_not_report_a_change() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let document = build(move || {
        let (selected, set_selected) = create_signal(None::<usize>);
        set_selected.set(Some(1));
        view! {
            <Select
                options={view! {
                    <ChoiceOption label="To do" />
                    <ChoiceOption label="Done" />
                }}
                selected={selected}
                label="Status"
                on_change={move |index| sink.borrow_mut().push(index)}
            />
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        changes.borrow().is_empty(),
        "a select that only followed its prop must not report a user change"
    );
}
