use super::*;

#[test]
fn a_block_opened_from_a_tab_replaces_it() {
    let (mut fixture, opened) = editor();
    let linked = Uuid::new_v4();

    show(&mut fixture, opened, None, None);
    show(&mut fixture, linked, None, Some(opened));

    assert_eq!(
        fixture.shown(),
        vec![linked],
        "a block opened from a tab takes that tab over"
    );

    fixture.test.click("workspace.back");
    fixture.settle();

    assert_eq!(
        fixture.shown(),
        vec![opened],
        "the tab it replaced is still behind it in the history"
    );
}
