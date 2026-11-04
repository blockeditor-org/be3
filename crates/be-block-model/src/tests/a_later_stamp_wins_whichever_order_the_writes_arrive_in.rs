use super::*;

#[test]
fn a_later_stamp_wins_whichever_order_the_writes_arrive_in() {
    let early: Edit = View::SCROLL.set(ObjectId::ROOT, &1, stamp(3, 1)).into();
    let late: Edit = View::SCROLL.set(ObjectId::ROOT, &2, stamp(4, 1)).into();
    let mut forward = Document::new(&View::default());
    forward.apply(&early);
    forward.apply(&late);
    let mut backward = Document::new(&View::default());
    backward.apply(&late);
    backward.apply(&early);

    assert_eq!(*forward.root().scroll, 2);
    assert_eq!(forward, backward);
    assert_eq!(
        forward.root().scroll.next(0, uuid::Uuid::nil()),
        stamp(5, 0)
    );
}
