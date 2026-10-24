use std::collections::BTreeMap;

use crate::{
    CanvasComponent, CanvasContent, CanvasEntity, CanvasPoint, CanvasShape, CanvasTransform,
    InfiniteCanvasOperation,
};
use be_block::{ChildChange, Root};
use database_types::database::DatabaseValue;
use uuid::Uuid;

fn entity(shape: CanvasShape, components: Vec<CanvasComponent>) -> CanvasEntity {
    CanvasEntity {
        components,
        ..CanvasEntity::new(
            shape,
            CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
        )
    }
}

fn component(schema: Uuid, values: &[(Uuid, DatabaseValue)]) -> CanvasComponent {
    CanvasComponent {
        schema_id: schema,
        values: values.iter().cloned().collect::<BTreeMap<_, _>>(),
    }
}

fn child(content: &mut CanvasContent, change: ChildChange) {
    let edit = content.root().child_edit(change).unwrap();
    content.apply(&edit);
}

#[test]
fn canvas_children_are_removed_repointed_or_merged() {
    let [shown, old, new, linked] = std::array::from_fn(|_| Uuid::new_v4());
    let [kept, moved] = std::array::from_fn(|_| Uuid::new_v4());
    let tagged = entity(
        CanvasShape::Rectangle,
        vec![
            component(
                old,
                &[
                    (kept, DatabaseValue::String("old".into())),
                    (moved, DatabaseValue::String("moved".into())),
                ],
            ),
            component(new, &[(kept, DatabaseValue::String("new".into()))]),
            component(linked, &[(kept, DatabaseValue::Block(shown))]),
        ],
    );
    let mut content = CanvasContent::default();
    for entity in [
        CanvasEntity {
            block: Some(shown),
            ..entity(CanvasShape::Editor, Vec::new())
        },
        tagged,
    ] {
        let edit = crate::Canvas::edit_in(
            &content,
            &InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
            },
        );
        content.apply(&edit);
    }
    assert_eq!(
        be_block::BlockContent::references(&content),
        [shown, old, new, linked]
    );

    child(&mut content, ChildChange::Replace { old, new });
    let entities = content.root().entities();
    let components = &entities[1].components;
    assert_eq!(components.len(), 2);
    let merged = &components[0];
    assert_eq!(merged.schema_id, new);
    assert_eq!(merged.values[&kept], DatabaseValue::String("new".into()));
    assert_eq!(merged.values[&moved], DatabaseValue::String("moved".into()));

    child(&mut content, ChildChange::Delete(shown));
    let entities = content.root().entities();
    assert_eq!(entities.len(), 1);
    assert!(entities[0].components[1].values.is_empty());
}
