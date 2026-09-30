use block_editor_beui::{BarAction, TopBar};

use super::*;

fn placed(fixture: &Fixture, id: Uuid) -> Option<block_editor_beui::ChildPlacement> {
    fixture
        .test
        .children()
        .iter()
        .find(|placement| Uuid::from_bytes(placement.block_id) == id)
        .copied()
}

fn top_bar(fixture: &Fixture, id: Uuid) -> Option<TopBar> {
    placed(fixture, id).map(|placement| placement.top_bar)
}

fn bar(fixture: &mut Fixture, id: Uuid, action: BarAction) {
    let child = placed(fixture, id).expect("the block is on show").child;
    fixture.test.child_bar(child, action);
    fixture.settle();
}

fn tap(fixture: &mut Fixture, test_id: &str) {
    fixture.test.click(test_id);
    fixture.settle();
}

#[test]
fn a_phone_shows_one_file_at_a_time_and_the_dock_bar_goes_back_or_switches() {
    let (mut fixture, first) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    let second = Uuid::new_v4();

    show(&mut fixture, first, None);
    assert_eq!(
        top_bar(&fixture, first),
        Some(TopBar::Phone { more: false }),
        "an opened block fills the phone and leaves its bar to the dock"
    );

    show(&mut fixture, second, None);
    assert_eq!(fixture.shown(), vec![second], "one file is shown at a time");

    tap(&mut fixture, "dock.back");
    assert!(
        placed(&fixture, second).is_none(),
        "back leaves the file for the list of files"
    );

    show(&mut fixture, second, None);
    tap(&mut fixture, "dock.switch");
    assert!(fixture.test.shown("dock.switcher.home"));
    fixture
        .test
        .snapshot("a_phone_lists_its_open_files_to_switch_between");
    tap(&mut fixture, "dock.switcher.tab.2");
    assert_eq!(
        fixture.shown(),
        vec![first],
        "choosing a file shows that file"
    );
    assert_eq!(fixture.focused(), Some(first));

    tap(&mut fixture, "dock.switch");
    tap(&mut fixture, "dock.switcher.close.2");
    assert_eq!(
        fixture.shown(),
        vec![second],
        "closing the file on show moves to the one opened before it"
    );
    assert_eq!(fixture.open_tabs(), 0, "a phone draws no tab bars");
    fixture.test.back();
    fixture.settle();
    assert!(
        !fixture.test.shown("dock.switcher.home"),
        "the back gesture closes the switcher"
    );
    assert_eq!(fixture.shown(), vec![second], "and leaves the file on show");

    tap(&mut fixture, "workspace.more");
    assert_eq!(
        top_bar(&fixture, second),
        Some(TopBar::Phone { more: true }),
        "the dock bar's more button opens the block's own sheet"
    );
    bar(&mut fixture, second, BarAction::CloseMore);
    assert_eq!(top_bar(&fixture, second), Some(TopBar::Phone { more: false }));

    bar(&mut fixture, second, BarAction::Details);
    assert!(fixture.test.shown("workspace.details.rename"));
    fixture
        .test
        .snapshot("a_phone_shows_a_files_details_in_a_sheet");
}
