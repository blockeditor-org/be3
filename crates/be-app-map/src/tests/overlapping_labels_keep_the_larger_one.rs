use super::*;

#[test]
fn overlapping_labels_keep_the_larger_one() {
    let mut editor = editor();

    serve_tiles_with(&mut editor, || {
        labelled_tile(&[
            ("Harbour", "city", 2048, 2048),
            ("Quay", "village", 2080, 2060),
        ])
    });

    assert!(editor.shown("map.label.Harbour.1"));
    assert!(!editor.shown("map.label.Quay.1"));
    editor.snapshot("overlapping_labels_keep_the_larger_one");
}
