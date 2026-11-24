use super::*;

#[test]
fn the_latest_gift_of_an_editor_stands_and_asking_again_keeps_its_count() {
    let mut inner = up(3, ChildMode::Live);
    inner.nested = placing(Vec::new(), 0);
    inner.nested.instance = 9;
    let mut children = placing(vec![up(1, ChildMode::Live), up(2, ChildMode::Live), inner], 0);

    // Each editor's latest ask stands; one for another app's editor goes to the children that
    // app places, and one for an app shown nowhere is left over.
    let mut asked = vec![
        gift(5, 1, GiftTo::Child(1)),
        gift(5, 1, GiftTo::Child(2)),
        gift(9, 4, GiftTo::Child(7)),
        gift(6, 1, GiftTo::Child(1)),
    ];
    children.ask(&mut asked);
    assert_eq!(asked, vec![gift(6, 1, GiftTo::Child(1))]);
    assert_eq!(
        children.gifts,
        vec![Pending {
            editor: EditorInstanceId(1),
            to: GiftTo::Child(2),
            up: None,
        }]
    );
    assert_eq!(children.shown[2].nested.gifts.len(), 1);

    // Asked again for the same child, the gift keeps counting from when its child came up.
    let up_at = Duration::from_secs(10);
    children.gifts[0].up = Some((up_at, 0));
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(2))]);
    assert_eq!(children.gifts[0].up, Some((up_at, 0)));
    // Asked for another, it starts again.
    children.ask(&mut vec![gift(5, 1, GiftTo::Child(1))]);
    assert_eq!(children.gifts[0].up, None);
    assert_eq!(children.gifts[0].to, GiftTo::Child(1));
}
