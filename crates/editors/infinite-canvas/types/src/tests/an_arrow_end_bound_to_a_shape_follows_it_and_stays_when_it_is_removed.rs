use crate::{
    BINDING_GAP, CanvasArrowhead, CanvasBinding, CanvasContent, CanvasEntity, CanvasPoint,
    CanvasShape, CanvasTransform, InfiniteCanvasOperation,
};

fn run(content: &mut CanvasContent, operation: InfiniteCanvasOperation) {
    let edit = crate::Canvas::edit_in(content, &operation);
    content.apply(&edit);
}

fn end_of(content: &CanvasContent, arrow: &CanvasEntity) -> CanvasPoint {
    let entities = content.root().entities();
    let held = entities
        .iter()
        .find(|entity| entity.id == arrow.id)
        .unwrap();
    held.world_points().last().unwrap().at()
}

fn near(a: CanvasPoint, b: CanvasPoint) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
}

#[test]
fn an_arrow_end_bound_to_a_shape_follows_it_and_stays_when_it_is_removed() {
    let mut target = CanvasEntity::new(
        CanvasShape::Rectangle,
        CanvasTransform::new(
            CanvasPoint::new(200.0, 0.0),
            CanvasPoint::new(100.0, 50.0),
            0.0,
        ),
    );
    let mut arrow = CanvasEntity::path(&[CanvasPoint::new(0.0, 0.0), CanvasPoint::new(200.0, 0.0)]);
    arrow.style.end_arrowhead = CanvasArrowhead::Arrow;
    arrow.bindings[1] = Some(CanvasBinding {
        entity: target.id,
        anchor: CanvasPoint::default(),
    });
    let mut content = CanvasContent::default();
    for entity in [target.clone(), arrow.clone()] {
        run(
            &mut content,
            InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
            },
        );
    }
    assert!(
        near(
            end_of(&content, &arrow),
            CanvasPoint::new(150.0 - BINDING_GAP, 0.0)
        ),
        "the end stops short of the rectangle's near edge"
    );

    target.transform.center = CanvasPoint::new(200.0, 300.0);
    run(
        &mut content,
        InfiniteCanvasOperation::Update {
            entities: vec![target.clone()],
        },
    );
    let followed = end_of(&content, &arrow);
    let toward = CanvasPoint::new(200.0, 300.0);
    let length = toward.x.hypot(toward.y);
    let edge = CanvasPoint::new(toward.x * 275.0 / toward.y, 275.0);
    assert!(
        near(
            followed,
            CanvasPoint::new(
                edge.x - toward.x / length * BINDING_GAP,
                edge.y - toward.y / length * BINDING_GAP,
            ),
        ),
        "the end follows the moved rectangle, stopping short of its top edge, got {followed:?}"
    );

    run(
        &mut content,
        InfiniteCanvasOperation::Remove {
            ids: vec![target.id],
        },
    );
    let [kept] = content.root().entities().try_into().unwrap();
    assert_eq!(kept.bindings, [None, None]);
    assert!(near(kept.world_points()[1].at(), followed));
}
