use accesskit::Role;

use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::Container;

#[test]
fn on_a_narrow_screen_picking_a_date_moves_on_to_the_time() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let field = NodeRef::new();
    let named = field.clone();
    let document = build(move || {
        view! {
            <Container>
                {move |_| view! {
                    <List spacing=0.0>
                        <DateTimeField
                            @node_ref=&named
                            value={Some(DateTime::new(Date::new(2026, 9, 27), Time::new(9, 0)))}
                            on_change={move |value| sink.borrow_mut().push(value)}
                        />
                    </List>
                }}
            </Container>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(360.0, 700.0));
    harness.frame(Vec::new());
    let find = |harness: &Harness, role: Role, label: &str| {
        harness
            .document()
            .accessibility
            .iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(label))
            .map(|(id, _)| id)
    };

    let rect = harness.rect(field.get());
    harness.click(pos2(rect.left() + 20.0, rect.top() + 3.0));
    harness.frame(Vec::new());
    assert!(
        find(&harness, Role::Tab, "Date").is_some(),
        "a narrow picker shows tabs"
    );
    assert!(
        find(&harness, Role::ListBox, "Time").is_none(),
        "and starts on the date"
    );

    let day = find(&harness, Role::GridCell, "Monday, 28 September 2026").expect("the day");
    harness.click(harness.center(day));
    harness.frame(Vec::new());
    assert!(
        find(&harness, Role::ListBox, "Time").is_some(),
        "picking a day shows the times"
    );

    let ten = find(&harness, Role::ListBoxOption, "10:00").expect("ten o'clock");
    harness.click(harness.center(ten));
    harness.frame(Vec::new());

    assert_eq!(
        reported.borrow().last(),
        Some(&Some(DateTime::new(
            Date::new(2026, 9, 28),
            Time::new(10, 0)
        )))
    );
}
