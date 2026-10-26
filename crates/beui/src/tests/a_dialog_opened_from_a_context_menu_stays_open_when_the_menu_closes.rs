use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_dialog_opened_from_a_context_menu_stays_open_when_the_menu_closes() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let surface = NodeRef::new();
    let dismissed = create_signal(0);
    let (document, [_]) = toolbar_of(move || {
        let open = create_signal(false);
        let items = view! {
            <unstyled::MenuItem label="Inspect" />
        };
        [view! {
            <List spacing=0.0>
                <ContextMenu items on_select={move |_path| open.set(true)}>
                    <MenuRegion @node_ref=&region />
                </ContextMenu>
                <Dialog
                    open={open}
                    title="Inspect"
                    on_dismiss={move || dismissed.update(|dismissed| *dismissed += 1)}
                >
                    <Text color=Color32::WHITE @node_ref={&surface}>"Details"</Text>
                </Dialog>
            </List>
        }]
    });
    let region = region.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Input::PointerMoved(pos)]);
    harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        dismissed.get_untracked(),
        0,
        "closing the menu must not dismiss the dialog its item opened"
    );
    assert!(
        harness
            .document()
            .node_rect(surface.get())
            .is_some_and(|rect| rect.is_positive()),
        "the dialog the menu item opened must still be showing"
    );
}
