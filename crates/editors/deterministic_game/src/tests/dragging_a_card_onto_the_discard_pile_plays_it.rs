use game_api::{Gesture, Spot};

use super::*;

#[test]
fn dragging_a_card_onto_the_discard_pile_plays_it() {
    let actions = dealt();
    let play = a_plain_play(&actions);
    let Some(Gesture::Drag {
        from: Spot::Card { pile, card },
        ..
    }) = play.gesture
    else {
        panic!("a card is played by dragging it");
    };
    let mut editor = editor_after(CRAZY_8S.to_vec(), actions.clone());
    editor.snapshot("the_table_before_dragging_a_card");

    let from = on_the_card(&editor, &format!("game.card.{pile}.{card}"));
    let to = editor.editor.rect_of("game.pile.1").center();
    editor.editor.drag(from, to);
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), actions.len() + 1);
    assert_eq!(played.last().map(|last| &last.action), Some(&play.effect));
    editor.snapshot("dragging_a_card_onto_the_discard_pile_plays_it");
}
