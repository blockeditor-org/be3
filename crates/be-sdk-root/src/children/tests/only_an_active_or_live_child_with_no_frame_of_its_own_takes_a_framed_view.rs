use super::*;

fn view(children: Vec<ChildPlacement>) -> ChildPlacements {
    ChildPlacements {
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        children,
        occluders: Vec::new(),
    }
}

fn numbered(id: u64, mode: ChildMode) -> ChildPlacement {
    let mut placement = placed(rect((0.0, 0.0), (50.0, 50.0)), mode);
    placement.child = ChildId(id);
    placement
}

#[test]
fn only_an_active_or_live_child_with_no_frame_of_its_own_takes_a_framed_view() {
    let mut framed = placed(rect((0.0, 0.0), (300.0, 200.0)), ChildMode::Live);
    framed.own_frame = true;
    let screens = [child_screen(&framed, EditorInstanceId(1), 1.0, 1.0)];
    let none = HashSet::default();
    let main = |_| ViewId::MAIN;
    let shows = |_, _, _| true;
    let rules = |frame_revoked, revoked| Framing {
        frame_revoked,
        revoked,
        content: &main,
        shows: &shows,
    };
    let mut own_frame = numbered(2, ChildMode::Active);
    own_frame.own_frame = true;
    let mut empty = numbered(3, ChildMode::Live);
    empty.rect = rect((0.0, 0.0), (0.0, 50.0));
    let placed = [view(vec![
        numbered(1, ChildMode::Passive),
        own_frame,
        empty,
        numbered(4, ChildMode::InPlace),
        numbered(5, ChildMode::Active),
        numbered(6, ChildMode::Live),
    ])];
    // The first that may: passive, in place, framed or empty children never take it.
    let taking = framing(&placed, &screens, &rules(&none, &none));
    assert_eq!(
        taking,
        [(EditorInstanceId(1), ViewId::MAIN, ChildId(5), [1; 16])]
    );

    // One taken back from the frame, or as an active child, leaves it to the next.
    let mut revoked = HashSet::default();
    revoked.insert((EditorInstanceId(1), ViewId::MAIN, ChildId(5)));
    let taking = framing(&placed, &screens, &rules(&revoked, &none));
    assert_eq!(taking[0].2, ChildId(6));
    let taking = framing(&placed, &screens, &rules(&none, &revoked));
    assert_eq!(taking[0].2, ChildId(6));

    // A view drawn with no frame has none to take.
    let bare = [child_screen(
        &placed_at_main(),
        EditorInstanceId(1),
        1.0,
        1.0,
    )];
    assert_eq!(framing(&placed, &bare, &rules(&none, &none)), []);

    // Nor does a view that does not hold the editor's content, or a child that draws nothing.
    let docked = |_| ViewId(4);
    let elsewhere = Framing {
        content: &docked,
        ..rules(&none, &none)
    };
    assert_eq!(framing(&placed, &screens, &elsewhere), []);
    let hidden = |_, _, id: ChildId| id != ChildId(5);
    let drawn = Framing {
        shows: &hidden,
        ..rules(&none, &none)
    };
    assert_eq!(framing(&placed, &screens, &drawn)[0].2, ChildId(6));
}

fn placed_at_main() -> ChildPlacement {
    placed(rect((0.0, 0.0), (300.0, 200.0)), ChildMode::Live)
}
