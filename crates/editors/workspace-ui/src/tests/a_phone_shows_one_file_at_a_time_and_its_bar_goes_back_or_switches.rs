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

fn bar(fixture: &mut Fixture, id: Uuid, action: BarAction) {
    let child = placed(fixture, id).expect("the block is on show").child;
    fixture.test.child_bar(child, action);
    fixture.settle();
}

#[test]
fn a_phone_shows_one_file_at_a_time_and_its_bar_goes_back_or_switches() {
    let (mut fixture, first) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    let second = Uuid::new_v4();

    show(&mut fixture, first, None);
    let placement = placed(&fixture, first).expect("an opened block fills the phone");
    assert_eq!(placement.top_bar, TopBar::Phone { open_files: 1 });

    show(&mut fixture, second, None);
    assert_eq!(fixture.shown(), vec![second], "one file is shown at a time");
    assert_eq!(
        placed(&fixture, second).map(|placement| placement.top_bar),
        Some(TopBar::Phone { open_files: 2 })
    );

    bar(&mut fixture, second, BarAction::Back);
    assert!(
        placed(&fixture, second).is_none(),
        "back leaves the file for the list of files"
    );

    show(&mut fixture, second, None);
    bar(&mut fixture, second, BarAction::Switch);
    assert!(fixture.test.shown("workspace.switcher.files"));
    fixture
        .test
        .snapshot("a_phone_lists_its_open_files_to_switch_between");
    fixture.test.click("workspace.switcher.tab.2");
    fixture.settle();
    assert_eq!(
        fixture.shown(),
        vec![first],
        "choosing a card shows that file"
    );
    assert_eq!(fixture.focused(), Some(first));

    bar(&mut fixture, first, BarAction::Switch);
    fixture.test.click("workspace.switcher.close.2");
    fixture.settle();
    assert_eq!(
        fixture.shown(),
        vec![second],
        "closing the file on show moves to the one opened before it"
    );
    assert_eq!(
        placed(&fixture, second).map(|placement| placement.top_bar),
        Some(TopBar::Phone { open_files: 1 })
    );

    bar(&mut fixture, second, BarAction::Details);
    assert!(fixture.test.shown("workspace.details.rename"));
    fixture
        .test
        .snapshot("a_phone_shows_a_files_details_in_a_sheet");
}
