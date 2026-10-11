use super::*;

#[test]
fn an_open_menu_is_withheld_from_the_block_under_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1000.0, 600.0)));

    show(&mut fixture, opened, None);
    let resting = fixture.test.occluders().to_vec();
    assert!(
        resting
            .iter()
            .all(|occluder| occluder.rect.width.min(occluder.rect.height) <= 16.0),
        "a workspace with no menu open withholds only the split handle along the block's edge"
    );

    fixture.test.click("workspace.access");
    fixture.settle();

    let occluder = fixture
        .test
        .occluders()
        .iter()
        .find(|occluder| !resting.contains(occluder))
        .copied()
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
