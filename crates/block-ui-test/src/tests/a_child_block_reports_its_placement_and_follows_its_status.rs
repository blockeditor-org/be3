use super::*;

#[test]
fn a_child_block_reports_its_placement_and_follows_its_status() {
    let mut test = editor();
    test.run();

    let placement = match test.children() {
        [placement] => *placement,
        children => panic!("expected one placed child, got {}", children.len()),
    };
    assert_eq!(placement.block_id, SLIDE.into_bytes());
    assert_eq!(placement.block_type, SLIDE_TYPE.into_bytes());
    assert_eq!(placement.mode, ChildMode::Preview);
    assert_eq!(placement.rect.width, 320.0);
    assert_eq!(placement.rect.height, 180.0);

    test.run();
    assert_eq!(status(&test), "waiting");

    test.available_children();
    test.run();
    assert_eq!(status(&test), "available");
}
