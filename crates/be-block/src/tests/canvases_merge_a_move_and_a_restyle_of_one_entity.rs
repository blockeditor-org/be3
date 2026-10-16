use crate::Merge;
use crate::canvas::{
    CanvasColor, CanvasContent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint,
    CanvasTransform, InfiniteCanvasOperation,
};
use be_commit::MergeResult;
use uuid::Uuid;

fn run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    let mut changed = content.clone();
    let edit = content.root().edit_for(&operation);
    changed.apply(&edit);
    changed
}

#[test]
fn canvases_merge_a_move_and_a_restyle_of_one_entity() {
    let original = CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    };
    let base = run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: original.clone(),
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
    restyled.style.foreground = CanvasColor::Rgba {
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
