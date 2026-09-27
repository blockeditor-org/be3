use accesskit::{Action, ActionRequest, Role, TreeId};

use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn a_date_segment_reads_and_steps_as_a_spin_button() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                value={Some(DateTime::new(Date::new(2026, 9, 27), Time::MIDNIGHT))}
                parts=DateTimeParts::Date
                label="Due"
                on_change={move |value: Option<DateTime>| {
                    sink.borrow_mut().push(value.map(|value| value.date))
                }}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", TALL_VIEWPORT);
    let (month_id, month) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::SpinButton && node.label() == Some("Month"))
        .expect("the month segment is a spin button");
    assert_eq!(month.value(), Some("September"));
    assert_eq!(month.numeric_value(), Some(9.0));
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Group && node.label() == Some("Due"))
    );

    harness.context.accessibility_action(ActionRequest {
        action: Action::Increment,
        target_tree: TreeId::ROOT,
        target_node: *month_id,
        data: None,
    });
    harness.frame(Vec::new());

    assert_eq!(*reported.borrow(), [Some(Date::new(2026, 10, 27))]);
}
