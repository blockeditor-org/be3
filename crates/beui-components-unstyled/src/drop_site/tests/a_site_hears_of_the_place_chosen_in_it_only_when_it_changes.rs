use super::*;

#[test]
fn a_site_hears_of_the_place_chosen_in_it_only_when_it_changes() {
    let fixture = Fixture::new();
    let (_, list) = fixture.node(Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)));
    let place = move |name: &'static str, y: f32| {
        fixed(Candidate::new(
            name,
            Rect::from_min_size(pos2(0.0, y), vec2(100.0, 20.0)),
            list,
        ))
    };
    let site = drop_site_on(
        &fixture.board,
        Func::new(|_: u32| true),
        Func::new(move |_: u32| closest([place("top", 0.0), place("bottom", 80.0)])),
        Callback::new(|_: (u32, &'static str)| {}),
    );
    let heard = Rc::new(std::cell::Cell::new(0));
    {
        let heard = Rc::clone(&heard);
        create_effect(move || {
            site.over.track();
            heard.set(heard.get() + 1);
        });
    }
    flush();
    let before = heard.get();

    fixture.board.lift(carry(1_u32), at(pos2(50.0, 10.0)));
    flush();
    assert_eq!(heard.get(), before + 1, "the lift chooses the top");
    for y in [11.0, 12.0, 12.0, 13.0] {
        fixture.track(pos2(50.0, y));
        flush();
    }
    assert_eq!(
        heard.get(),
        before + 1,
        "points that keep the choice tell nothing"
    );

    fixture.track(pos2(50.0, 90.0));
    flush();
    assert_eq!(heard.get(), before + 2, "a new choice is told once");
    assert_eq!(site.over.get_untracked(), Some("bottom"));
    fixture.release();
}
