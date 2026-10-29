use accesskit::Role;

use super::*;
use crate::datetime::Date;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn tapping_a_calendar_day_picks_it() {
    let _root = crate::reactive::RootScope::new();
    let picked = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Calendar
                selected={Binding::new(None, move |date| picked.update(|picked| picked.push(date)))}
                today={Some(Date::new(2026, 1, 1))}
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

    assert_eq!(picked.get_untracked(), [Some(Date::new(2026, 1, 16))]);
}
