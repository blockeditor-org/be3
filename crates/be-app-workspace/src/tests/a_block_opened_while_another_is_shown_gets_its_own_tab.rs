use super::*;

#[test]
fn a_block_opened_while_another_is_shown_gets_its_own_tab() {
    let (mut fixture, opened) = editor();
    let linked = Uuid::new_v4();

    show(&mut fixture, opened, None);
    assert_eq!(fixture.open_tabs(), 1);

    show(&mut fixture, linked, Some(opened));

    assert_eq!(
        fixture.shown(),
        vec![linked],
        "the block opened second is on show"
    );
    assert_eq!(
        fixture.open_tabs(),
        2,
        "the first block kept its own tab beside the new one"
    );
}
