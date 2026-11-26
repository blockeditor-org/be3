use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_context_menu_is_built_only_while_it_is_open_or_leaving() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let open = create_signal(false);
    let (document, [menu]) = toolbar_of(move || {
        let items = view! {
            <unstyled::MenuItem label="Rename" />
            <unstyled::MenuItem label="Delete" />
        };
        [view! {
            <ContextMenu items open>
                <MenuRegion @node_ref=&region />
            </ContextMenu>
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let built = |harness: &Harness| {
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(menu)
            .built()
    };
    assert!(!built(&harness), "a closed context menu builds no menu");

    for time in ["first", "second"] {
        let pos = harness.center(region.get());
        press_secondary(&mut harness, pos);
        assert!(open.get_untracked());
        assert!(built(&harness), "opened the {time} time, its menu is built");

        with_installed(harness.document_mut(), |_| open.set(false));
        harness.settle();
        assert!(!built(&harness), "and closed again, it is dropped");
    }

    // Opened again before its close has finished, it stays built.
    let pos = harness.center(region.get());
    press_secondary(&mut harness, pos);
    with_installed(harness.document_mut(), |_| open.set(false));
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| open.set(true));
    harness.frame(Vec::new());
    assert!(built(&harness), "reopened as it closes, its menu is built");
    harness.settle();
    assert!(
        built(&harness),
        "and stays built once the close it cut short is over"
    );
}
