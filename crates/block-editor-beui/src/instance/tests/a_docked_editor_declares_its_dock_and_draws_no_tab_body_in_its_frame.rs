use super::*;
use block_plugin_api::{DockMessage, DockTabPath};

#[test]
fn a_docked_editor_declares_its_dock_and_draws_no_tab_body_in_its_frame() {
    let mut session = docked_session();

    show_docked(&mut session, &[(View::MAIN, 0)], true);

    let declared: Vec<_> = dock_messages(&mut session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Declare(declare) => Some(declare),
            _ => None,
        })
        .collect();
    let [declare] = declared.as_slice() else {
        panic!("a docked editor should declare its dock once, not {declared:?}");
    };
    assert_eq!(declare.view.view, ViewId::MAIN);
    let declaration = declare
        .declaration
        .as_ref()
        .expect("the docked editor's dock is declared, not withdrawn");
    declaration
        .validate_own()
        .expect("a docked editor declares only its own tabs");
    let tabs: Vec<_> = declaration
        .tabs
        .iter()
        .map(|tab| {
            (
                tab.tab.clone(),
                tab.key.as_str(),
                tab.title.as_str(),
                tab.closable,
            )
        })
        .collect();
    assert_eq!(
        tabs,
        [
            (DockTabPath::own(1), "1", "One", false),
            (DockTabPath::own(2), "2", "Two", true),
        ]
    );
    assert_eq!(BODIES.with(std::cell::Cell::get), 0);
    assert!(session.placed_children(ViewId::MAIN).is_empty());
}
