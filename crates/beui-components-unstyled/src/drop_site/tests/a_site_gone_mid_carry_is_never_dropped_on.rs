use super::*;

#[test]
fn a_site_gone_mid_carry_is_never_dropped_on() {
    let fixture = Fixture::new();
    let (_, left) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let (_, right) = fixture.node(Rect::from_min_size(pos2(300.0, 0.0), vec2(100.0, 100.0)));
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
    let scope = Scope::new();
    let on_left = scope.run(|| site(left, "left"));
    let on_right = site(right, "right");

    fixture.board.lift(carry(1_u32), at(pos2(50.0, 50.0)));
    assert_eq!(on_left.over.get_untracked(), Some("left"));

    drop(scope);
    assert_eq!(
        on_right.over.get_untracked(),
        Some("right"),
        "the site gone, the carry chooses again where the pointer was"
    );
    fixture.release();
    assert_eq!(*dropped.borrow(), vec!["right"], "and never drops on the one gone");
}
