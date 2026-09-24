use super::*;

#[test]
fn an_open_menu_is_withheld_from_the_block_under_it() {
    let (mut fixture, opened) = editor();

    show(&mut fixture, opened, None);
    assert!(
        fixture.test.occluders().is_empty(),
        "a workspace with no menu open withholds nothing from the block it shows"
    );

    fixture.test.click("workspace.access");
    fixture.settle();

    let occluder = fixture
        .test
        .occluders()
        .first()
        .expect("an open menu must be withheld from the block editor beneath it");
    assert!(
        occluder.rect.width > 0.0 && occluder.rect.height > 0.0,
        "the occluder covers the rectangle the menu was laid out at"
    );
    assert!(
        occluder.after as usize >= fixture.test.children().len(),
        "the menu is withheld from every block the workspace placed under it"
    );
}
