use accesskit::Role;

use super::*;
use crate::datetime::Date;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_calendar_keeps_its_focus_between_its_limits() {
    let _root = crate::reactive::RootScope::new();
    let picked = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Calendar
                selected={Binding::new(None, move |date| picked.update(|picked| picked.push(date)))}
                today={Some(Date::new(2026, 5, 10))}
                min={Some(Date::new(2026, 5, 8))}
                max={Some(Date::new(2026, 5, 12))}
                focused=true
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

    assert_eq!(
        picked.get_untracked(),
        [Some(Date::new(2026, 5, 8)), Some(Date::new(2026, 5, 12))]
    );
    let outside = harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == Role::GridCell && node.label() == Some("Thursday, 7 May 2026"))
        .expect("the day before the limit is in the grid");
    assert!(outside.is_disabled());
}
