use game_api::{Gesture, Spot};

use super::*;

#[test]
fn clicking_a_card_then_the_discard_pile_plays_it() {
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

    let on_it = on_the_card(&editor, &format!("game.card.{pile}.{card}"));
    editor.click_at(on_it);
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len());
    editor.snapshot("clicking_a_card_selects_it");

    editor.click("game.pile.1");
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), actions.len() + 1);
    assert_eq!(played.last().map(|last| &last.action), Some(&play.effect));
}
