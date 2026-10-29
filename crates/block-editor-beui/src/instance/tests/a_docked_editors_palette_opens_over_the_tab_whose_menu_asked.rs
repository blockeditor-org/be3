use super::*;

#[test]
fn a_docked_editors_palette_opens_over_the_tab_whose_menu_asked() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 0)], true);
    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[1, 2], &[2], Some(2))]);
    let shown = [(View::MAIN, 0), (View::dock(ViewId(2)), 1)];
    show_docked(&mut session, &shown, true);

    session.pick_menu(ViewId(2), "editor.palette".to_owned());
    show_docked(&mut session, &shown, true);
    show_docked(&mut session, &shown, true);

    let frames = session.frame(&[(ViewId::MAIN, 0), (ViewId(2), 0)]);
    assert!(
        !frames[1].floating.is_empty(),
        "the palette floats over the tab whose menu opened it"
    );
    assert!(
        frames[0].floating.is_empty(),
        "nothing opens in the docked frame, which the host does not show"
    );
}
