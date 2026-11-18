use super::*;

#[test]
fn each_pointer_carries_its_own_item_and_escape_puts_down_every_one() {
    let fixture = Fixture::new();
    let (_, left) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let (_, right) = fixture.node(Rect::from_min_size(pos2(500.0, 0.0), vec2(100.0, 100.0)));
    let dropped = Rc::new(RefCell::new(Vec::<(u32, &'static str)>::new()));
    // Each site takes one payload, as a slider takes its own thumb.
    let site = |node: NodeRef, name: &'static str, takes: u32| {
        let dropped = Rc::clone(&dropped);
        drop_site_on(
            &fixture.board,
            Func::new(move |payload: u32| payload == takes),
            Func::new(move |_: u32| {
                fixed(Candidate::new(
                    name,
                    Rect::from_min_size(pos2(40.0, 40.0), vec2(20.0, 20.0)),
                    node,
                ))
            }),
            Callback::new(move |(payload, drop): (u32, &'static str)| {
                dropped.borrow_mut().push((payload, drop))
            }),
        )
    };
    let site_left = site(left, "left", 1);
    let site_right = site(right, "right", 2);
    let finger = PointerId::Touch(beui_view::types::TouchId {
        device: 0,
        finger: 3,
    });
    let order = |_| (Vec::new(), WindowId::MAIN);

    assert!(fixture.board.lift(carry(1_u32), at(pos2(50.0, 50.0))).is_some());
    assert!(
        fixture.board.lift(carry(3_u32), at(pos2(60.0, 60.0))).is_none(),
        "a pointer carries one item at a time"
    );
    assert!(
        fixture
            .board
            .lift(carry(2_u32), at(pos2(550.0, 50.0)).by(finger))
            .is_some(),
        "another pointer lifts its own"
    );
    assert_eq!(site_left.over.get_untracked(), Some("left"));
    assert_eq!(site_right.over.get_untracked(), Some("right"));
    Driver::release(&*fixture.board, finger, false, &order);
    assert_eq!(
        *dropped.borrow(),
        vec![(2, "right")],
        "the finger drops only its own"
    );
    assert_eq!(fixture.carried(), Some(1), "and the mouse's is still carried");

    assert!(fixture.board.lift(carry(2_u32), at(pos2(550.0, 50.0)).by(finger)).is_some());
    assert!(fixture.board.cancel(), "Escape");
    assert_eq!(fixture.carried(), None, "puts down every carry");
    assert_eq!(*dropped.borrow(), vec![(2, "right")], "dropping none");
    assert_eq!(site_left.over.get_untracked(), None);
    assert_eq!(site_right.over.get_untracked(), None);
}
