use super::*;

#[test]
fn a_docked_editor_builds_a_tab_body_only_where_the_host_shows_it_and_keeps_it() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 0)], true);
    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[1, 2], &[1], Some(1))]);
    show_docked(&mut session, &[(View::MAIN, 0)], true);
    assert_eq!(BODIES.with(std::cell::Cell::get), 0);

    show_docked(
        &mut session,
        &[(View::MAIN, 0), (View::dock(ViewId(1)), 1)],
        true,
    );
    let placed = session.placed_children(ViewId(1)).to_vec();
    let [child] = placed.as_slice() else {
        panic!("tab 1's body should place its child block, not {placed:?}");
    };
    assert_eq!(child.content.block_id(), Some(DOCKED_CHILD.into_bytes()));
    assert_eq!(child.docks, Some(1));
    assert_eq!(BODIES.with(std::cell::Cell::get), 1);

    show_docked(
        &mut session,
        &[(View::MAIN, 0), (View::dock(ViewId(1)), 2)],
        true,
    );
    show_docked(&mut session, &[(View::MAIN, 0)], true);
    show_docked(
        &mut session,
        &[(View::MAIN, 0), (View::dock(ViewId(1)), 3)],
        true,
    );

    let placed = session.placed_children(ViewId(1)).to_vec();
    assert_eq!(
        placed.iter().map(|placed| placed.child).collect::<Vec<_>>(),
        [child.child],
        "the child keeps its id while its tab moves and comes back"
    );
    assert_eq!(
        BODIES.with(std::cell::Cell::get),
        1,
        "the body is built once and kept while the root holds its tab"
    );
    assert!(session.placed_children(ViewId::MAIN).is_empty());
}
