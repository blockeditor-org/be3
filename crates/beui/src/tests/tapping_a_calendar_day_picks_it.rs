use accesskit::Role;

use super::*;
use crate::datetime::Date;
use crate::reactive::view;
use crate::styled::Calendar;

#[test]
fn tapping_a_calendar_day_picks_it() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let document = build(move || {
        view! {
            <Calendar
                selected=None
                today={Some(Date::new(2026, 1, 1))}
                on_change={move |date| sink.borrow_mut().push(date)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let day = harness
        .document()
        .accessibility
        .iter()
        .find(|(_, node)| {
            node.role() == Role::GridCell && node.label() == Some("Friday, 16 January 2026")
        })
        .map(|(id, _)| id)
        .expect("the day is in the grid");
    let centre = harness.center(day);

    harness.touch(TouchPhase::Start, centre);
    harness.touch(TouchPhase::End, centre);
    harness.frame(Vec::new());

    assert_eq!(*picked.borrow(), [Date::new(2026, 1, 16)]);
}
