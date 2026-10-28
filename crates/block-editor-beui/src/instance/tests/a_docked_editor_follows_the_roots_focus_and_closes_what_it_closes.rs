use super::*;
use block_plugin_api::{DockClosedTab, DockTabPath, DockUpdate};

#[test]
fn a_docked_editor_follows_the_roots_focus_and_closes_what_it_closes() {
    let mut session = docked_session();
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], true);

    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[2, 1], &[2], Some(2))]);
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], true);
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    assert_eq!(layout.focused(), Some(2));
    assert!(layout.contains(&1) && layout.contains(&2));
    assert!(!CLOSED.with(std::cell::Cell::get));

    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Close(DockClosedTab {
            region: EditorRegion::Frame,
            tab: DockTabPath::own(2),
        })]);
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], true);

    assert!(CLOSED.with(std::cell::Cell::get));
}
