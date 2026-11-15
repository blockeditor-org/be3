use super::*;

#[test]
fn another_pointer_neither_moves_nor_drops_a_carry() {
    let fixture = Fixture::new();
    let (_, left) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let (_, right) = fixture.node(Rect::from_min_size(pos2(500.0, 0.0), vec2(100.0, 100.0)));
    let dropped = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let site = |node: NodeRef, name: &'static str| {
        let dropped = Rc::clone(&dropped);
        drop_site_on(
            &fixture.board,
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                fixed(Candidate::new(
                    name,
                    Rect::from_min_size(pos2(40.0, 40.0), vec2(20.0, 20.0)),
                    node,
                ))
            }),
            Callback::new(move |(_, drop): (u32, &'static str)| dropped.borrow_mut().push(drop)),
        )
    };
    let site_left = site(left, "left");
    let _site_right = site(right, "right");
    let finger = PointerId::Touch(beui_view::types::TouchId {
        device: 0,
        finger: 3,
    });
    let order = |_| (Vec::new(), WindowId::MAIN);

    fixture.board.lift(carry(1_u32), at(pos2(50.0, 50.0)));
    Driver::track(&*fixture.board, at(pos2(550.0, 50.0)).by(finger), &order);
    assert_eq!(
        site_left.over.get_untracked(),
        Some("left"),
        "a finger's move leaves a carry the mouse holds"
    );
    Driver::release(&*fixture.board, finger, false, &order);
    assert_eq!(fixture.carried(), Some(1), "and its release does not put it down");

    fixture.track(pos2(550.0, 50.0));
    fixture.release();
    assert_eq!(*dropped.borrow(), vec!["right"], "the mouse moves and drops it");
}
