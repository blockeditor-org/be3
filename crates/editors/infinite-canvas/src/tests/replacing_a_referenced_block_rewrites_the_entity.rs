use super::*;

#[test]
fn replacing_a_referenced_block_rewrites_the_entity() {
    let old = Uuid::from_u128(0x0100_0000_0000_4000_8000_0000_0000_0011);
    let new = Uuid::from_u128(0x0200_0000_0000_4000_8000_0000_0000_0012);
    let mut referencing = entity(Uuid::from_u128(3));
    referencing.shape = CanvasShape::Editor;
    referencing.block = Some(old);
    let mut editor = editor(std::slice::from_ref(&referencing));

    assert!(editor.replace_child(old, new));

    let embeds = entities(&editor)
        .into_iter()
        .map(|entity| (entity.shape, entity.block))
        .collect::<Vec<_>>();
    assert_eq!(embeds, [(CanvasShape::Editor, Some(new))]);
}
