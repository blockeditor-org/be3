use accesskit::Role;

use super::*;
use crate::datetime::Date;
use crate::reactive::view;
use crate::styled::Calendar;

#[test]
fn a_calendar_keeps_its_focus_between_its_limits() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let document = build(move || {
        view! {
            <Calendar
                selected=None
                today={Some(Date::new(2026, 5, 10))}
                min={Some(Date::new(2026, 5, 8))}
                max={Some(Date::new(2026, 5, 12))}
                focused=true
                on_change={move |date| sink.borrow_mut().push(date)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    for _ in 0..3 {
        harness.key(Key::ArrowLeft, Modifiers::NONE);
    }
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::PageDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*picked.borrow(), [Date::new(2026, 5, 8), Date::new(2026, 5, 12)]);
    let outside = harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == Role::GridCell && node.label() == Some("Thursday, 7 May 2026"))
        .expect("the day before the limit is in the grid");
    assert!(outside.is_disabled());
}
