use super::*;
use crate::datetime::Date;
use crate::reactive::view;
use crate::styled::Calendar;

#[test]
fn a_calendar_reaches_a_distant_year_from_its_years_view() {
    let picked = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let document = build(move || {
        view! {
            <Calendar
                selected={Some(Date::new(2026, 1, 15))}
                today={Some(Date::new(2026, 1, 1))}
                focused=true
                on_change={move |date| sink.borrow_mut().push(date)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::PageUp, Modifiers::NONE);
    harness.key(Key::PageUp, Modifiers::NONE);
    for _ in 0..4 {
        harness.key(Key::ArrowLeft, Modifiers::NONE);
    }
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(
        picked.borrow().is_empty(),
        "choosing a year and a month only shows them"
    );
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*picked.borrow(), [Date::new(1982, 4, 15)]);
}
