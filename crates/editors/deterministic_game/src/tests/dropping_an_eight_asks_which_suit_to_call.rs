use game_api::{Gesture, Spot};

use super::*;

#[test]
fn dropping_an_eight_asks_which_suit_to_call() {
    let (actions, eights) = (1..)
        .map(|opponent| {
            let actions = dealt_against(Uuid::from_u128(opponent));
            let eights: Vec<GameActionOption> = offered(CRAZY_8S, &actions, ACCOUNT)
                .into_iter()
                .filter(|option| option.label.contains("and call"))
                .collect();
            (actions, eights)
        })
        .find(|(_, eights)| !eights.is_empty())
        .expect("some deal gives the first player an eight");
    let Some(Gesture::Drag {
        from: Spot::Card { pile, card },
        ..
    }) = eights[0].gesture
    else {
        panic!("an eight is played by dragging it");
    };
    let mut editor = editor_after(CRAZY_8S.to_vec(), actions.clone());

    let from = on_the_card(&editor, &format!("game.card.{pile}.{card}"));
    let to = editor.rect_of("game.pile.1").center();
    editor.drag(from, to);
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len());
    let called = eights
        .iter()
        .position(|option| option.label.ends_with("call Hearts"))
        .expect("an eight can call hearts");
    let choice = offered(CRAZY_8S, &actions, ACCOUNT)
        .iter()
        .position(|option| option.effect == eights[called].effect)
        .expect("the eight is offered");
    editor.snapshot("dropping_an_eight_asks_which_suit_to_call");

    editor.click(&format!("game.choice.{choice}"));
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), actions.len() + 1);
    assert_eq!(
        played.last().map(|last| &last.action),
        Some(&eights[called].effect)
    );
}
