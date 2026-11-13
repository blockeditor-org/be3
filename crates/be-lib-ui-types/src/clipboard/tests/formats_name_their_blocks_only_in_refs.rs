use super::*;

#[test]
fn formats_name_their_blocks_only_in_refs() {
    let (first, second, canvas) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let block = |id| ClipBlock {
        id,
        organization: None,
        block_type: Uuid::from_u128(9),
        title: "Notes".to_owned(),
    };
    let blocks = Blocks {
        blocks: vec![block(first), block(second)],
    };
    let mut item = blocks.encode();
    assert_eq!(item.refs, vec![first, second]);
    assert_eq!(item.decode::<Blocks>(), Some(blocks.clone()));

    let (moved, other) = (Uuid::from_u128(4), Uuid::from_u128(5));
    item.refs = vec![moved, other];
    assert_eq!(
        item.decode::<Blocks>(),
        Some(Blocks {
            blocks: vec![block(moved), block(other)],
        }),
        "the data holds no id: whoever translates the refs translates the blocks"
    );
    item.refs.pop();
    assert_eq!(item.decode::<Blocks>(), None, "refs that do not fit fail");

    let items = CanvasItems {
        canvas,
        blocks: vec![second],
        entities: b"[]".to_vec(),
    };
    let item = items.encode();
    assert_eq!(item.refs, vec![canvas, second]);
    assert_eq!(item.decode::<CanvasItems>(), Some(items));

    let stored = ClipItem::from_stored(formats::CANVAS_ITEMS, &item.stored());
    assert_eq!(
        stored,
        Some(item),
        "a system clipboard's custom type keeps the refs"
    );
}
