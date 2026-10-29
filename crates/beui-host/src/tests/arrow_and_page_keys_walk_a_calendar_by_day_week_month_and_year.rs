use super::*;
use crate::datetime::Date;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn arrow_and_page_keys_walk_a_calendar_by_day_week_month_and_year() {
    let _root = crate::reactive::RootScope::new();
    let picked = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Calendar
                selected={Binding::new(Some(Date::new(2026, 1, 30)), move |date| picked.update(|picked| picked.push(date)))}
                today={Some(Date::new(2026, 1, 1))}
                focused=true
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
    assert!(
        picked.with_untracked(|picked| picked.is_empty()),
        "moving the focus picks nothing"
    );
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(picked.get_untracked(), [Some(Date::new(2025, 3, 3))]);
}
