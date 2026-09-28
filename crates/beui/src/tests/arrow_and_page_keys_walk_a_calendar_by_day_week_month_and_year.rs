use super::*;
use crate::datetime::Date;
use crate::reactive::view;
use crate::styled::Calendar;

#[test]
fn arrow_and_page_keys_walk_a_calendar_by_day_week_month_and_year() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let document = build(move || {
        view! {
            <Calendar
                selected={Some(Date::new(2026, 1, 30))}
                today={Some(Date::new(2026, 1, 1))}
                focused=true
                on_change={move |date| sink.borrow_mut().push(date)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::PageDown, Modifiers::NONE);
    harness.key(Key::PageUp, Modifiers::SHIFT);
    harness.key(Key::Home, Modifiers::NONE);
    assert!(picked.borrow().is_empty(), "moving the focus picks nothing");
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*picked.borrow(), [Date::new(2025, 3, 3)]);
}
