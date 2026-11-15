use super::*;

#[test]
fn a_fast_release_lands_where_its_path_leads() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = closest([slot(node, 1, 0.0), slot(node, 2, 400.0)]).thrown();
    let release = |velocity: Vec2| {
        evaluate(
            &spec,
            &Evaluation {
                point: at(pos2(60.0, 10.0)),
                velocity,
                released: true,
                placed: &placed,
            },
            &DragFrame::default(),
        )
    };
    assert_eq!(
        release(vec2(100.0, 0.0)).drop,
        Some(1),
        "a slow release lands nearest"
    );
    assert_eq!(
        release(vec2(THROW_SPEED * 4.0, 0.0)).drop,
        Some(2),
        "a fast one carries on along its path"
    );
}
