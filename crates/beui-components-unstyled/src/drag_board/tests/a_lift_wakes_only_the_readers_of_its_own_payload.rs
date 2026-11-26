use super::*;
use beui_view::drag::Carry;
use beui_view::reactive::{create_effect, flush};
use beui_view::types::input::PointerId;

/// A reader of one payload's carry reruns on that payload's lift and put-down alone, and a
/// reader of every payload of a type on those of its type alone.
#[test]
fn a_lift_wakes_only_the_readers_of_its_own_payload() {
    let fixture = Fixture::new();
    let board = Rc::new(fixture.board);
    let runs = Rc::new(RefCell::new(HashMap::<&str, usize>::default()));
    fixture.scope.run(|| {
        for (name, payload) in [("one", 1_u32), ("two", 2_u32)] {
            let board = Rc::clone(&board);
            let runs = Rc::clone(&runs);
            create_effect(move || {
                board.carries(&payload);
                *runs.borrow_mut().entry(name).or_default() += 1;
            });
        }
        let board = Rc::clone(&board);
        let runs = Rc::clone(&runs);
        create_effect(move || {
            board.carried::<&'static str>();
            *runs.borrow_mut().entry("words").or_default() += 1;
        });
    });
    flush();
    let ran = |runs: &Rc<RefCell<HashMap<&str, usize>>>| {
        let runs = runs.borrow();
        ["one", "two", "words"].map(|name| runs.get(name).copied().unwrap_or(0))
    };
    assert_eq!(ran(&runs), [1, 1, 1]);

    board.lift(Carry::new(1_u32), at_point(pos2(10.0, 10.0)));
    flush();
    assert_eq!(
        ran(&runs),
        [2, 1, 1],
        "lifting 1 wakes the reader of 1 alone"
    );

    let order = |_| (vec![0], WindowId::MAIN);
    Driver::release(&*board, PointerId::Mouse, false, &order);
    flush();
    assert_eq!(
        ran(&runs),
        [3, 1, 1],
        "putting 1 down wakes the reader of 1 alone"
    );

    board.lift(Carry::new("word"), at_point(pos2(10.0, 10.0)));
    flush();
    assert_eq!(
        ran(&runs),
        [3, 1, 2],
        "lifting a word wakes the reader of every word alone"
    );
}
