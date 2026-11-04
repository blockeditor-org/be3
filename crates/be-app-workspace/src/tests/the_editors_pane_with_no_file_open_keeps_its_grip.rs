use super::*;

#[test]
fn the_editors_pane_with_no_file_open_keeps_its_grip() {
    let (fixture, _) = profiled_sized(Some(WIDE), None);
    assert!(fixture.says("No file open"));
    assert!(
        fixture.test.shown("dock.grip.editors"),
        "the empty editors pane still has a grip to move or split it by"
    );
}
