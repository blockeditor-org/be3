use super::*;

#[test]
fn a_lifted_item_is_offered_to_every_site_that_takes_it_and_dropped_on_the_one_chosen() {
    let fixture = Fixture::new();
    let (_, left) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let (_, right) = fixture.node(Rect::from_min_size(pos2(500.0, 0.0), vec2(100.0, 100.0)));
    let dropped = Rc::new(RefCell::new(Vec::<(u32, &'static str)>::new()));
    let site = |node: NodeRef, name: &'static str, takes: u32| {
        let dropped = Rc::clone(&dropped);
        drop_site_on(
            &fixture.board,
            Func::new(move |payload: u32| payload == takes || takes == 0),
            Func::new(move |_: u32| {
                closest([fixed(Candidate::new(
                    name,
                    Rect::from_min_size(pos2(40.0, 40.0), vec2(20.0, 20.0)),
                    node,
                ))])
            }),
            Callback::new(move |(payload, drop): (u32, &'static str)| {
                dropped.borrow_mut().push((payload, drop))
            }),
        )
    };
    let on_left = site(left, "left", 0);
    let on_right = site(right, "right", 0);
    let picky = site(right, "picky", 9);

    fixture.board.lift(carry(7_u32), at(pos2(60.0, 50.0)));
    assert_eq!(fixture.carried(), Some(7));
    assert_eq!(on_left.over.get_untracked(), Some("left"));
    assert_eq!(on_right.over.get_untracked(), None);

    fixture.track(pos2(450.0, 50.0));
    assert_eq!(on_left.over.get_untracked(), None, "the site left hears it");
    assert_eq!(on_right.over.get_untracked(), Some("right"));
    assert_eq!(picky.over.get_untracked(), None, "a site that does not take it is never chosen");
    assert!(dropped.borrow().is_empty(), "nothing is written before the release");

    fixture.release();
    assert_eq!(*dropped.borrow(), vec![(7, "right")]);
    assert_eq!(on_right.over.get_untracked(), None);
    assert_eq!(fixture.carried(), None);

    fixture.board.lift(carry(8_u32), at(pos2(60.0, 50.0)));
    assert!(Driver::cancel(&*fixture.board), "escape puts it down");
    assert_eq!(dropped.borrow().len(), 1, "and drops nothing");
}
