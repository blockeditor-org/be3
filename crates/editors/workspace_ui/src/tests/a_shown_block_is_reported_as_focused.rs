use super::*;

#[test]
fn a_shown_block_is_reported_as_focused() {
    let (mut fixture, opened) = editor();
    let second = Uuid::new_v4();

    show(&mut fixture, opened, None);
    assert_eq!(fixture.focused(), Some(opened));
    assert_eq!(
        fixture.host.focused_block().block_type,
        <FileTree as Block>::TYPE_ID
    );

    show(&mut fixture, second, Some(opened));

    assert_eq!(
        fixture.shown(),
        vec![second],
        "the tab that was opened second is the one on show"
    );
    assert_eq!(fixture.focused(), Some(second));
    assert_eq!(fixture.host.focused_block().via, vec![opened]);
}
