use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn an_open_pick_leaves_the_tabs_where_they_were() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1000.0, 600.0)));
    show(&mut fixture, opened, None);
    let placed = |fixture: &Fixture| {
        fixture
            .test
            .children()
            .iter()
            .find(|placement| placement.content.block_id() == Some(opened.into_bytes()))
            .map(|placement| placement.rect)
            .expect("the shown block is placed")
    };
    let before = placed(&fixture);

    fixture
        .test
        .request_pick(3, filter(), BlockLocation::Root, None);
    fixture.settle();

    assert!(
        fixture
            .test
            .document()
            .find_test_id("picker.more")
            .is_some(),
        "the pick's menu is open"
    );
    assert_eq!(
        placed(&fixture),
        before,
        "an open pick takes no room from the workspace under it"
    );
}
