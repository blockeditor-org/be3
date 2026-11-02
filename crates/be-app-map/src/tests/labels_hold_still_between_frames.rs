use super::*;

#[test]
fn labels_hold_still_between_frames() {
    let mut editor = editor();
    serve_tiles_with(&mut editor, || {
        labelled_tile(&[
            ("Harbour", "city", 2048, 2048),
            ("Lighthouse", "village", 1024, 1024),
            ("Reef", "village", 3072, 3072),
        ])
    });
    let shown = |editor: &BeuiTest<MapApp>| {
        ["Harbour", "Lighthouse", "Reef"]
            .into_iter()
            .flat_map(|name| {
                (1..=16).map(move |occurrence| format!("map.label.{name}.{occurrence}"))
            })
            .filter(|id| editor.shown(id))
            .collect::<Vec<_>>()
    };
    let first = shown(&editor);
    assert_eq!(first.len(), 48);

    for _ in 0..3 {
        editor.step(Vec::new());
        assert_eq!(shown(&editor), first);
    }
}
