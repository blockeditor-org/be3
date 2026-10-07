use game_api::{Gesture, Spot};

use super::*;

#[test]
fn a_played_card_flies_from_the_hand_to_the_discard_pile() {
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
    let held = format!("game.card.{pile}.{card}");
    let id = Game::load(CRAZY_8S)
        .expect("the test module is a game")
        .show(&actions, ACCOUNT)
        .expect("the game answers")
        .board
        .hand[card as usize]
        .id;
    let flight = format!("game.flight.{}", id.0);
    let start = editor.rect_of(&held);

    editor.click_at(on_the_card(&editor, &held));
    editor.run();
    editor.click("game.pile.1");
    editor.run();

    assert_eq!(moves(&editor).len(), actions.len() + 1);
    let landing = editor.rect_of("game.pile.1");
    let flying = editor.rect_of(&flight);
    assert!(landing.center().y < flying.center().y && flying.center().y < start.center().y);
    editor.snapshot("a_played_card_flies_from_the_hand_to_the_discard_pile");

    editor.settle();
    assert!(!editor.shown(&flight));
}
