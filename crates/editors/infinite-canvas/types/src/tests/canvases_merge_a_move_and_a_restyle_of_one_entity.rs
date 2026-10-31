use crate::{
    CanvasColor, CanvasContent, CanvasEntity, CanvasPoint, CanvasShape, CanvasTransform,
    InfiniteCanvasOperation,
};
use be_block::Merge;
use be_commit::MergeResult;

fn run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    let mut changed = content.clone();
    let edit = crate::Canvas::edit_in(content, &operation);
    changed.apply(&edit);
    changed
}

#[test]
fn canvases_merge_a_move_and_a_restyle_of_one_entity() {
    let original = CanvasEntity::new(
        CanvasShape::Rectangle,
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
    );
    let base = run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: Box::new(original.clone()),
            pixels: None,
        },
    );
    let mut moved = original.clone();
    moved.transform.center = CanvasPoint::new(50.0, 0.0);
    let ours = run(
        &base,
        InfiniteCanvasOperation::Update {
            entities: vec![moved],
        },
    );
    let mut restyled = original;
    restyled.style.stroke_color = CanvasColor::Rgba {
        red: 255,
        green: 0,
        blue: 0,
        alpha: 255,
    };
    let theirs = run(
        &base,
        InfiniteCanvasOperation::Update {
            entities: vec![restyled.clone()],
        },
    );

    let MergeResult::Clean(merged) = CanvasContent::merge3(&base, &ours, &theirs) else {
        panic!("the two sides changed different fields of the entity");
    };

    let [entity] = merged.root().entities().try_into().unwrap();
    assert_eq!(entity.transform.center, CanvasPoint::new(50.0, 0.0));
    assert_eq!(entity.style, restyled.style);
}
