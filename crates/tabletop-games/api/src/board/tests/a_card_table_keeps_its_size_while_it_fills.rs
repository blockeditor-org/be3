use super::*;

fn backs(group: u32, count: u32) -> Vec<(ItemId, Sprite)> {
    (0..count)
        .map(|index| (ItemId::new(group, index), Sprite::CardBack))
        .collect()
}

#[test]
fn a_card_table_keeps_its_size_while_it_fills() {
    let empty = card_table(vec![
        Vec::new(),
        vec![Pile::stacked(0, "Deck", backs(1, 52))],
    ]);
    let dealt = card_table(vec![
        vec![Pile::fanned(2, "P2", backs(2, 8))],
        vec![Pile::stacked(0, "Draw pile", backs(1, 35))],
    ]);

    assert_eq!((empty.width, empty.height), (dealt.width, dealt.height));
    let deck: Vec<&Area> = empty
        .items
        .iter()
        .filter(|item| item.spot == Some(Spot::Pile(0)))
        .map(|item| &item.area)
        .collect();
    assert_eq!(deck.len(), 53);
    assert!(deck.iter().all(|area| **area == *deck[0]));
    let fan: Vec<i32> = dealt
        .items
        .iter()
        .filter(|item| matches!(item.spot, Some(Spot::Card { pile: 2, .. })))
        .map(|item| item.area.x)
        .collect();
    assert_eq!(fan.len(), 8);
    assert!(fan.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(fan[7] + 70 - fan[0] <= 220);
}
