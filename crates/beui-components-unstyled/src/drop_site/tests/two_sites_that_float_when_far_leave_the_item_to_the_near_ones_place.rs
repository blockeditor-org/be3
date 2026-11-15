use super::*;

/// Kanban columns: each site's place is its column's slot, floating when far. The far
/// column's floating never takes the item from the near column's place.
#[test]
fn two_sites_that_float_when_far_leave_the_item_to_the_near_ones_place() {
    let fixture = Fixture::new();
    let (_, todo) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 300.0)));
    let (_, done) = fixture.node(Rect::from_min_size(pos2(500.0, 0.0), vec2(100.0, 300.0)));
    let column = |node: NodeRef, name: &'static str| {
        drop_site_on(
            &fixture.board,
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                closest([fixed(Candidate::new(
                    name,
                    Rect::from_min_size(pos2(40.0, 40.0), vec2(20.0, 20.0)),
                    node,
                ))])
                .when_far(floating())
            }),
            Callback::new(|_: (u32, &'static str)| {}),
        )
    };
    let todo = column(todo, "todo");
    let done = column(done, "done");

    fixture.board.lift(carry(1_u32), at(pos2(60.0, 50.0)));
    assert_eq!(todo.over.get_untracked(), Some("todo"), "the near column's place wins");
    assert_eq!(done.over.get_untracked(), None);

    fixture.track(pos2(540.0, 50.0));
    assert_eq!(todo.over.get_untracked(), None);
    assert_eq!(done.over.get_untracked(), Some("done"));

    fixture.track(pos2(300.0, 600.0));
    assert_eq!(todo.over.get_untracked(), None, "far from both, it floats");
    assert_eq!(done.over.get_untracked(), None);
    fixture.release();
}
