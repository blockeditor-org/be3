use super::*;
use crate::canvas::{CanvasComponent, CanvasContent, InfiniteCanvasOperation};

#[test]
#[ignore = "a canvas component is an object per insert, so the same schema added on both sides shows up twice"]
fn a_component_added_to_one_entity_on_both_sides_is_kept_once() {
    let (schema, name, size) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let original = rectangle();
    let base = canvas_run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: original.clone(),
        },
    );
    let with = |field: Uuid, value: DatabaseValue| {
        let mut entity = original.clone();
        entity.components = vec![CanvasComponent {
            schema_id: schema,
            values: [(field, value)].into_iter().collect(),
        }];
        InfiniteCanvasOperation::Update {
            entities: vec![entity],
        }
    };
    let ours = canvas_run(&base, with(name, DatabaseValue::String("box".to_owned())));
    let theirs = canvas_run(&base, with(size, DatabaseValue::Number(3.0)));

    let (merged, _) = merged(&base, &ours, &theirs);

    let [entity] = merged.root().entities().try_into().expect("one entity");
    assert_eq!(entity.components.len(), 1);
    assert_eq!(entity.components[0].schema_id, schema);
    assert_eq!(entity.components[0].values.len(), 2);
}
