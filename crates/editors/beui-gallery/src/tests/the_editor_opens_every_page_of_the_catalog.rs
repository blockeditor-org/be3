use super::*;

#[test]
fn the_editor_opens_every_page_of_the_catalog() {
    let host = EditorHost::default();
    let mut test = BeuiTest::<BeuiGalleryApp>::new(OwnedEditor::new(host, Uuid::new_v4()))
        .with_size(Vec2::new(1000.0, 1600.0));
    test.run();
    for title in beui_demo::page_titles() {
        let row = format!("demo.catalog.{title}");
        let document = test.document();
        let at = document
            .find_test_id(&row)
            .and_then(|node| document.node_rect(node))
            .unwrap_or_else(|| panic!("the catalog shows {title}"))
            .center();
        drop(document);
        test.click(at);
        test.run();
        assert!(
            test.document().find_test_id(&row).is_some(),
            "the catalog stays beside {title}"
        );
    }
}
