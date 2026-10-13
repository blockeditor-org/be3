use super::*;

#[test]
fn carets_arrive_after_the_edits_they_point_into() {
    let (simulation, _left, right) = simulated("ab");
    typed(&right, 2, "cd");
    let at = {
        let read = right.read().unwrap();
        Position::at(&*read, 3)
    };
    simulation.write().unwrap().publish(
        Side::Right,
        Caret {
            anchor: at,
            focus: at,
        },
        Instant::now(),
    );
    assert_eq!(simulation.read().unwrap().seen(Side::Left), None);

    send(&simulation, Side::Right);

    let seen = simulation
        .read()
        .unwrap()
        .seen(Side::Left)
        .expect("the caret arrived");
    assert_eq!(text(&simulation, Side::Left), "abcd");
    let read = SideDocument::new(&simulation, Side::Left);
    let read = read.read().unwrap();
    assert_eq!(seen.focus.resolve(&*read), 3);
}
