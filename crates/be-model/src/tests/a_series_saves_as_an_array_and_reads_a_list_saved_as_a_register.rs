use super::*;

#[test]
fn a_series_saves_as_an_array_and_reads_a_list_saved_as_a_register() {
    let mut document = trace(&[1.0]);
    document.apply(&Edit(vec![grown(&document, &[1.0, 2.0])]));
    let reloaded = Document::<Trace>::from_bytes(&document.to_bytes()).expect("the bytes decode");
    assert_eq!(reloaded, document);
    assert!(reloaded.session_state().is_empty());

    let old = Document::new(&OldTrace {
        points: vec![3.0, 4.0],
    });
    let read = Document::<Trace>::from_bytes(&old.to_bytes()).expect("the bytes decode");
    assert_eq!(*read.root().points, [3.0, 4.0]);
}
