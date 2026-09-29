use super::*;

#[test]
fn labels_stay_while_deeper_tiles_load() {
    let mut editor = editor();
    serve_tiles_with(&mut editor, || {
        labelled_tile(&[("Harbour", "city", 2048, 2048)])
    });
    let shown = |editor: &BeuiTest<MapApp>| {
        (1..=16)
            .filter(|occurrence| editor.shown(&format!("map.label.Harbour.{occurrence}")))
            .count()
    };

    for _ in 0..5 {
        editor.click("map.zoom-in");
        editor.run();
        assert!(shown(&editor) > 0);
    }

    assert!(editor.has_requests());
    assert!(shown(&editor) > 0);
}
