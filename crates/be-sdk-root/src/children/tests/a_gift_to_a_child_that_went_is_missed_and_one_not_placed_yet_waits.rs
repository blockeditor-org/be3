use super::*;

#[test]
fn a_gift_to_a_child_that_went_is_missed_and_one_not_placed_yet_waits() {
    let mut children = placing(vec![up(1, ChildMode::Live)], 0);
    let now = Duration::from_secs(1);

    // A child not placed yet: the layout that places it may come after the ask, so it waits.
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(4))]);
    assert_eq!(give(&mut children, PLACER, &[], now), (None, Vec::new()));
    assert_eq!(children.gifts.len(), 1);

    // A child that was placed and went is missed, which its editor hears.
    let placed: HashSet<_> = [(EditorInstanceId(1), ChildId(1))].into_iter().collect();
    children.depart(placed.iter().copied().chain([(EditorInstanceId(1), ChildId(3))]).collect());
    children.depart(placed.clone());
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(3))]);
    assert_eq!(
        give(&mut children, PLACER, &[], now),
        (None, vec![(5, 1, PlacerEvent::GiftMissed(3))])
    );
    assert!(children.gifts.is_empty());

    // Placed again, it has not departed.
    children.depart(placed.iter().copied().chain([(EditorInstanceId(1), ChildId(3))]).collect());
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(3))]);
    assert_eq!(give(&mut children, PLACER, &[], now), (None, Vec::new()));
    assert!(
        children.gifts.is_empty(),
        "placed as a kind the root shows no editor of, a gift to it ends unmissed"
    );

    // Two gifts that can be given at once: only one takes the keyboard.
    let mut children = placing(vec![up(1, ChildMode::Live), up(2, ChildMode::Live)], 0);
    children.placers.push((LayerId(101), EditorInstanceId(2), ViewId::MAIN));
    children.shown[1].editor = EditorInstanceId(2);
    children.ask(&mut vec![
        gift(5, 1, GiftTo::Child(1)),
        gift(5, 2, GiftTo::Child(2)),
    ]);
    let (given, _) = give(&mut children, PLACER, &[], now);
    assert_eq!(given, Some(LayerId(201)));
    assert_eq!(children.gifts.len(), 0, "the other ends: the keyboard is not within its editor");
}
