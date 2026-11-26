use super::*;

#[test]
fn a_place_in_another_window_is_never_chosen_by_the_pointer() {
    let fixture = Fixture::new();
    let (main, here) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let (_, there) = fixture.node(Rect::from_min_size(pos2(400.0, 0.0), vec2(100.0, 100.0)));
    let other = WindowId::next();
    let site = drop_site_on(
        &fixture.board,
        Func::new(|_: u32| true),
        Func::new(move |_: u32| {
            let place = |node: NodeRef, name: &'static str| {
                fixed(Candidate::new(
                    name,
                    Rect::from_min_size(pos2(40.0, 40.0), vec2(20.0, 20.0)),
                    node,
                ))
            };
            closest([place(here, "here"), place(there, "there")])
        }),
        Callback::new(|_: (u32, &'static str)| {}),
    );
    let windows = move |node: NodeId| match node == main {
        true => (Vec::new(), WindowId::MAIN),
        false => (Vec::new(), other),
    };

    fixture.board.lift(carry(1_u32), at(pos2(60.0, 50.0)));
    Driver::track(&*fixture.board, at(pos2(450.0, 50.0)), &windows);
    assert_eq!(
        site.over.get_untracked(),
        Some("here"),
        "a pointer in the main window does not reach a place in another, however near its \
         rect in that window's points"
    );

    Driver::track(
        &*fixture.board,
        DragPoint {
            window: other,
            ..at(pos2(450.0, 50.0))
        },
        &windows,
    );
    assert_eq!(
        site.over.get_untracked(),
        Some("there"),
        "and a pointer in that window chooses it"
    );
}
