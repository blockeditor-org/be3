use super::*;

#[test]
fn rereading_a_series_matches_a_fresh_read_after_any_edit() {
    let mut document = trace(&[1.0, 2.0]);
    let mut held = document.root();
    for wanted in [
        &[1.0, 2.0, 3.0][..],
        &[1.0, 2.0, 3.0, 4.0, 5.0],
        &[1.0, 9.0, 3.0, 4.0, 5.0, 6.0],
        &[1.0, 9.0],
        &[7.0],
        &[7.0, 8.0],
    ] {
        document.apply(&Edit(vec![grown(&document, wanted)]));
        held = document
            .reread(ObjectId::ROOT, held)
            .expect("the trace is the root");
        assert_eq!(*held.points, *wanted);
        assert_eq!(held, document.root());
    }
}
