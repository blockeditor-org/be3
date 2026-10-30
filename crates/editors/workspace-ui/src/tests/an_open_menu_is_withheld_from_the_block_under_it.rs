use super::*;

fn splitter(rect: Rect) -> bool {
    rect.width().min(rect.height()) < 10.0
}

#[test]
fn an_open_menu_is_withheld_from_the_block_under_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1000.0, 600.0)));

    show(&mut fixture, opened, None);
    assert!(
        fixture.test.occluders().iter().all(|occluder| splitter(occluder.rect)),
        "a workspace with no menu open withholds nothing from the block it shows but the reach of its splitters"
    );

    fixture.test.click("dock.editors.menu");
    fixture.settle();

    let occluder = fixture
        .test
        .occluders()
        .iter()
        .find(|occluder| !splitter(occluder.rect))
        .expect("an open menu must be withheld from the block editor beneath it");
    assert!(
        occluder.rect.width() > 0.0 && occluder.rect.height() > 0.0,
        "the occluder covers the rectangle the menu was laid out at"
    );
    assert!(
        occluder.after as usize >= fixture.test.children().len(),
        "the menu is withheld from every block the workspace placed under it"
    );
}
