use crate::{
    CanvasColor, CanvasContent, CanvasEntity, CanvasFill, CanvasPoint, CanvasShape,
    CanvasStrokeStyle, CanvasTextSize, CanvasTransform, InfiniteCanvasOperation,
};
use be_block::Merge;
use be_commit::MergeResult;

fn run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    let mut changed = content.clone();
    let edit = crate::Canvas::edit_in(content, &operation);
    changed.apply(&edit);
    changed
}

fn restyled(base: &CanvasContent, restyle: impl FnOnce(&mut CanvasEntity)) -> CanvasContent {
    let [mut entity] = base.root().entities().try_into().unwrap();
    restyle(&mut entity);
    run(
        base,
        InfiniteCanvasOperation::Update {
            entities: vec![entity],
        },
    )
}

#[test]
fn two_peers_restyling_different_fields_of_one_shape_keep_both() {
    let original = CanvasEntity::new(
        CanvasShape::Ellipse,
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(40.0, 20.0), 0.0),
    );
    let base = run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: Box::new(original),
        },
    );
    let red = CanvasColor::Rgba {
        red: 255,
        green: 0,
        blue: 0,
        alpha: 255,
    };
    let ours = restyled(&base, |entity| {
        entity.style.stroke_color = red;
        entity.style.fill = CanvasFill::Hatched;
        entity.style.text_size = CanvasTextSize::Large;
    });
    let theirs = restyled(&base, |entity| {
        entity.style.stroke_width = 6.0;
        entity.style.stroke_style = CanvasStrokeStyle::Dotted;
        entity.style.opacity = 0.5;
    });

    let MergeResult::Clean(merged) = CanvasContent::merge3(&base, &ours, &theirs) else {
        panic!("the two peers changed different style fields");
    };

    let [entity] = merged.root().entities().try_into().unwrap();
    assert_eq!(entity.style.stroke_color, red);
    assert_eq!(entity.style.fill, CanvasFill::Hatched);
    assert_eq!(entity.style.text_size, CanvasTextSize::Large);
    assert_eq!(entity.style.stroke_width, 6.0);
    assert_eq!(entity.style.stroke_style, CanvasStrokeStyle::Dotted);
    assert_eq!(entity.style.opacity, 0.5);
}
