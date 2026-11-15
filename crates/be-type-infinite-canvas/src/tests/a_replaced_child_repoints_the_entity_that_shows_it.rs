use be_block_content::{ChildChange, Root};

use super::*;

#[test]
fn a_replaced_child_repoints_the_entity_that_shows_it() {
    let old = Uuid::from_u128(0x0100_0000_0000_4000_8000_0000_0000_0011);
    let new = Uuid::from_u128(0x0200_0000_0000_4000_8000_0000_0000_0012);
    let mut content = CanvasContent::default();
    for block in [Some(old), None] {
        let entity = CanvasEntity {
            block,
            ..CanvasEntity::new(
                CanvasShape::Editor,
                CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
            )
        };
        let edit = Canvas::edit_in(
            &content,
            &InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
                pixels: None,
            },
        );
        content.apply(&edit);
    }

    let edit = content
        .root()
        .child_edit(ChildChange::Replace { old, new })
        .expect("the canvas takes the replacement");
    content.apply(&edit);

    let embeds = content
        .root()
        .entities()
        .into_iter()
        .map(|entity| (entity.shape, entity.block))
        .collect::<Vec<_>>();
    assert_eq!(
        embeds,
        [
            (CanvasShape::Editor, Some(new)),
            (CanvasShape::Editor, None)
        ]
    );
}
