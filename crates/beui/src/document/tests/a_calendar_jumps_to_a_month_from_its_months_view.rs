use super::*;
use crate::datetime::Date;
use crate::reactive::view;
use crate::styled::Calendar;

#[test]
fn a_calendar_jumps_to_a_month_from_its_months_view() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let document = build(move || {
        view! {
            <Calendar
                selected={Some(Date::new(2026, 1, 15))}
                focused=true
                on_change={move |date| sink.borrow_mut().push(date)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    for _ in 0..3 {
        harness.key(Key::Tab, Modifiers::SHIFT);
    }
    harness.key(Key::Enter, Modifiers::NONE);
    for _ in 0..3 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(picked.borrow().is_empty(), "choosing a month only shows it");
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*picked.borrow(), [Date::new(2026, 5, 15)]);
}
