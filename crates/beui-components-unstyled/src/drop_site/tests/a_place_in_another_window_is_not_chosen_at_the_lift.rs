use super::*;

#[test]
fn a_place_in_another_window_is_not_chosen_at_the_lift() {
    let fixture = Fixture::new();
    let (_, here) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    // A dock window the host opened, whose place sits under the pointer's coordinates in its
    // own window's points.
    let (window_node, there) =
        fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let other = WindowId::next();
    fixture.flat.1.borrow_mut().insert(window_node, other);
    let site = drop_site_on(
        &fixture.board,
        Func::new(|_: u32| true),
        Func::new(move |_: u32| {
            closest([
                fixed(Candidate::new(
                    "here",
                    Rect::from_min_size(pos2(0.0, 0.0), vec2(20.0, 20.0)),
                    here,
                )),
                fixed(Candidate::new(
                    "there",
                    Rect::from_min_size(pos2(50.0, 40.0), vec2(20.0, 20.0)),
                    there,
                )),
            ])
        }),
        Callback::new(|_: (u32, &'static str)| {}),
    );

    fixture.board.lift(carry(1_u32), at(pos2(60.0, 50.0)));
    assert_eq!(
        site.over.get_untracked(),
        Some("here"),
        "the lift knows the other place's window before the core tracks a point, so a pointer \
         in the main window does not choose it"
    );
}
