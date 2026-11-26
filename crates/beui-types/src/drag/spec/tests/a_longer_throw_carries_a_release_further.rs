use super::*;

#[test]
fn a_longer_throw_carries_a_release_further() {
    let root = RootScope::new();
    let node = origin(&root);
    let release = |spec: &DragSpec<u32>| {
        evaluate(
            spec,
            &Evaluation {
                point: at(pos2(60.0, 10.0)),
                velocity: vec2(1000.0, 0.0),
                released: true,
                placed: &placed,
            },
            &DragFrame::default(),
        )
        .drop
    };
    let spec = || closest([slot(node, 1, 0.0), slot(node, 2, 400.0)]).thrown();
    assert_eq!(
        release(&spec()),
        Some(1),
        "carried on for the default throw, it still lands nearer the first"
    );
    assert_eq!(
        release(&spec().thrown_for(0.4)),
        Some(2),
        "carried on for longer, it reaches the second"
    );
}
