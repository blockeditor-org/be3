use super::*;
use crate::datetime::Date;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_calendar_reaches_a_distant_year_from_its_years_view() {
    let _root = crate::reactive::RootScope::new();
    let picked = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Calendar
                selected={Binding::new(Some(Date::new(2026, 1, 15)), move |date| picked.update(|picked| picked.push(date)))}
                today={Some(Date::new(2026, 1, 1))}
                focused=true
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
        picked.with_untracked(|picked| picked.is_empty()),
        "choosing a year and a month only shows them"
    );
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(picked.get_untracked(), [Some(Date::new(1982, 4, 15))]);
}
