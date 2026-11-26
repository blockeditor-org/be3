use super::*;

#[test]
fn a_region_hears_whether_the_pointer_is_in_it() {
    let outputs = [display()];
    let mut input = input(Vec::new(), Vec::new());
    input.placed(pos2(400.0, 580.0));

    input.set(Vec::new(), vec![foot(false)]);
    assert!(
        region(&input).pointer,
        "a region set under the pointer finds it already in"
    );

    let _ = input.moved(pos2(400.0, 580.0), vec2(0.0, -200.0), &outputs);
    assert!(!region(&input).pointer);
}
