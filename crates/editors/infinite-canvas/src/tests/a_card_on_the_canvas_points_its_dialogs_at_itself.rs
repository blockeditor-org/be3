use super::*;
use block_editor_beui::be_block::BlockContent;
use counter_types::CounterContent;

#[test]
fn a_card_on_the_canvas_points_its_dialogs_at_itself() {
    let counter = Uuid::new_v4();
    let mut placed = entity(Uuid::from_u128(1));
    placed.shape = CanvasShape::Editor;
    placed.block = Some(counter);
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    let mut editor = open(
        &Canvas::with_entities([placed]),
        false,
        &[BlockInfo::new(
            counter,
            CounterContent::CONTENT_TYPE,
            BlockParent::Root,
        )],
    );
    editor.available_children();
    editor.run();

    let card = editor
        .children()
        .iter()
        .find(|child| child.content.block_id() == Some(counter.into_bytes()))
        .cloned()
        .expect("the card is placed");
    assert_eq!(
        card.anchor,
        Some(card.rect),
        "a card has no tab, so what it opens points at the card itself"
    );
}
