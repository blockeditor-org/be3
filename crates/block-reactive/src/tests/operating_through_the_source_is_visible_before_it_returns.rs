use super::*;

#[test]
fn operating_through_the_source_is_visible_before_it_returns() {
    let client = client();
    let source = BlockSource::new(client.create_block(Deck::default()), || {});
    let cards = source.project(|deck: &Deck| deck.cards().len());

    source.operate(add_card(0));
    assert_eq!(cards.get_untracked(), 1);
    source.operate(add_card(1));
    assert_eq!(cards.get_untracked(), 2);
}
