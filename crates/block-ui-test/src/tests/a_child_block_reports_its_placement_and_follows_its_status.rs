use super::*;

#[test]
fn a_child_block_reports_its_placement_and_follows_its_status() {
    let mut test = editor();
    test.run();

    let placement = match test.children() {
        [placement] => placement.clone(),
        children => panic!("expected one placed child, got {}", children.len()),
    };
    assert_eq!(
        placement.content,
        ChildContent::Block {
            block_id: SLIDE.into_bytes(),
            block_type: SLIDE_TYPE.into_bytes(),
            view_block: None,
        }
    );
    assert_eq!(placement.mode, ChildMode::Preview);
    assert_eq!(placement.rect.width, 320.0);
    assert_eq!(placement.rect.height, 180.0);

    test.run();
    assert_eq!(status(&test), "waiting");

    test.available_children();
    test.run();
    assert_eq!(status(&test), "available");
}
