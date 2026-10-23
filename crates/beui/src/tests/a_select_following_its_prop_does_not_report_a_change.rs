use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::ChoiceOption;

#[test]
fn a_select_following_its_prop_does_not_report_a_change() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let document = build_node(move || {
        let selected = create_signal(None::<usize>);
        selected.set(Some(1));
        view! {
            <Select
                options={view! {
                    <ChoiceOption label="To do" />
                    <ChoiceOption label="Done" />
                }}
                selected={Binding::new(selected, move |index| changes.update(|changes| changes.push(index)))}
                label="Status"
            />
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert!(
        changes.with_untracked(|changes| changes.is_empty()),
        "a select that only followed its prop must not report a user change"
    );
}
