use super::*;

#[test]
fn a_gift_waits_for_its_child_to_be_up_and_counts_from_there() {
    let mut opening = up(1, ChildMode::Live);
    opening.up = false;
    opening.layer = None;
    let mut children = placing(vec![opening, up(2, ChildMode::Live)], 0);
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(1))]);

    // However long its app takes to start, the gift waits for it.
    let start = Duration::from_secs(1);
    let late = start + GIFT_WAIT * 10;
    assert_eq!(give(&mut children, PLACER, &[], late), (None, Vec::new()));
    assert_eq!(children.gifts.len(), 1);

    // Up but not drawn yet, its clock starts: it waits `GIFT_WAIT` from there.
    children.shown[0].up = true;
    children.shown[0].layer = Some(LayerId(201));
    assert_eq!(give(&mut children, PLACER, &[LayerId(201)], late).0, None);
    assert_eq!(children.gifts[0].up, Some((late, 0)));
    assert_eq!(
        give(&mut children, PLACER, &[LayerId(201)], late + GIFT_WAIT / 2).0,
        None
    );
    assert_eq!(children.gifts.len(), 1);

    // Drawn, it is given.
    assert_eq!(
        give(&mut children, PLACER, &[], late + GIFT_WAIT / 2).0,
        Some(LayerId(201))
    );
    assert!(children.gifts.is_empty(), "a gift given ends");

    // One whose child never shows ends `GIFT_WAIT` after it came up...
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(2))]);
    let hidden = [LayerId(202)];
    assert_eq!(give(&mut children, PLACER, &hidden, late).0, None);
    assert_eq!(give(&mut children, PLACER, &hidden, late + GIFT_WAIT).0, None);
    assert!(children.gifts.is_empty());

    // ... or once its editor committed `GIFT_COMMITS` layouts since.
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(2))]);
    assert_eq!(give(&mut children, PLACER, &hidden, late).0, None);
    children.commits.insert(EditorInstanceId(1), GIFT_COMMITS - 1);
    assert_eq!(give(&mut children, PLACER, &hidden, late).0, None);
    assert_eq!(children.gifts.len(), 1);
    children.commits.insert(EditorInstanceId(1), GIFT_COMMITS);
    assert_eq!(give(&mut children, PLACER, &hidden, late).0, None);
    assert!(children.gifts.is_empty());

    // With the keyboard outside the editor's views and their children, a gift ends.
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(2))]);
    assert_eq!(give(&mut children, LayerId(1), &[], late).0, None);
    assert!(children.gifts.is_empty());
    // Within a child of the editor, it is given.
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(2))]);
    assert_eq!(
        give(&mut children, LayerId(201), &[], late).0,
        Some(LayerId(202))
    );

    // One whose child could not be shown ends; a gift to the child that shows a block is given
    // once one does.
    *children.shown[1].showing.borrow_mut() = Showing::Failed("it stopped".to_owned());
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(2))]);
    assert_eq!(give(&mut children, PLACER, &[], late).0, None);
    assert!(children.gifts.is_empty());
    children.ask(&mut vec![gift(5, 1, GiftTo::Block([1; 16]))]);
    assert_eq!(give(&mut children, PLACER, &[], late).0, Some(LayerId(201)));
}
