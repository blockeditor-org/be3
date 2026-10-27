use super::*;

#[test]
fn the_editor_shows_the_component_catalog() {
    let host = EditorHost::default();
    let mut test = BeuiTest::<BeuiGalleryApp>::new(OwnedEditor::new(host, Uuid::new_v4()))
        .with_size(Vec2::new(1000.0, 700.0));
    test.run();

    test.click("demo.catalog.Buttons");

    assert!(
        test.document()
            .find_test_id("demo.catalog.Buttons")
            .is_some()
    );
    test.snapshot("the_editor_shows_the_component_catalog");
}
