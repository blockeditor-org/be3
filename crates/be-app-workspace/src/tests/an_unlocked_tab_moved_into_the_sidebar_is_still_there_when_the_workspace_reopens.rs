use super::*;

#[test]
fn an_unlocked_tab_moved_into_the_sidebar_is_still_there_when_the_workspace_reopens() {
    let (fixture, _) = profiled_sized(Some(WIDE), None);
    let (mut fixture, places) = with_two_blocks(fixture);
    open_grip_menu(&mut fixture, "editors");
    fixture.click_text("Locked");
    fixture.drag_text("Minutes", BELOW_SIDEBAR);
    assert_eq!(placed_in_sidebar(&fixture, places.minutes), Some(true));

    let layout = profile(&fixture)
        .state("layout")
        .cloned()
        .expect("the layout is saved in the profile");
    let (mut reopened, _) = profiled_sized(Some(WIDE), Some(layout));
    reopened.settle();

    assert_eq!(
        placed_in_sidebar(&reopened, places.minutes),
        Some(true),
        "the unlocked tab stays where it was moved"
    );
    assert_eq!(
        placed_in_sidebar(&reopened, places.budget),
        Some(false),
        "the locked tab stays in the blocks area"
    );
}
