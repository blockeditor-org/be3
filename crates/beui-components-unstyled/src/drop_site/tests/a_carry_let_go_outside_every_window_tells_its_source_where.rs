use super::*;

#[test]
fn a_carry_let_go_outside_every_window_tells_its_source_where() {
    let fixture = Fixture::new();
    let heard = Rc::new(RefCell::new(Vec::<Pos2>::new()));
    let outside = |heard: &Rc<RefCell<Vec<Pos2>>>| {
        let heard = Rc::clone(heard);
        carry(1_u32).outside(move |point: DragPoint| heard.borrow_mut().push(point.pos))
    };

    fixture.board.lift(outside(&heard), at(pos2(50.0, 50.0)));
    fixture.track(pos2(80.0, 50.0));
    fixture.release();
    assert!(
        heard.borrow().is_empty(),
        "let go inside the window on no place, it is put down"
    );

    fixture.board.lift(outside(&heard), at(pos2(50.0, 50.0)));
    fixture.board.set_outside(true);
    fixture.track(pos2(900.0, 50.0));
    fixture.release();
    assert_eq!(
        *heard.borrow(),
        [pos2(900.0, 50.0)],
        "let go outside, its source hears where"
    );
}
