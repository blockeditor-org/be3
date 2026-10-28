use game_api::{Gesture, Spot};

use super::*;

#[test]
fn instant_motion_lands_cards_and_finishes_active_flights() {
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

    editor.click(on_the_card(&editor, &held));
    editor.click("game.pile.1");

    assert_eq!(moves(&editor).len(), actions.len() + 1);
    assert!(editor.shown(&flight));
    editor.set_preferences(block_editor_beui::Preferences {
        motion: block_editor_beui::Motion::Instant,
        ..Default::default()
    });
    editor.run();
    assert!(!editor.shown(&flight));
    editor.snapshot("instant_motion_lands_cards");
    assert_eq!(moves(&editor).len(), actions.len() + 1);
}
