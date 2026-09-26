use game_api::table::DRAW_PILE;
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{join, show};

#[test]
fn joining_and_dealing_are_clicks_on_the_deck() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let deck = Some(Gesture::Click(Spot::Pile(DRAW_PILE)));

    let joining = show(&[], first);
    assert_eq!(joining.actions.len(), 1);
    assert_eq!(joining.actions[0].gesture, deck);

    let mut actions = vec![join(&[], first)];
    let joined = join(&actions, second);
    actions.push(joined);
    let dealing = show(&actions, first);

    assert_eq!(dealing.actions.len(), 1);
    assert_eq!(dealing.actions[0].label, "Start the game");
    assert_eq!(dealing.actions[0].gesture, deck);
    assert!(dealing.columns.is_empty());
}
