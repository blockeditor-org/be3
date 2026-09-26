use super::*;
use crate::reactive::{NodeRef, Text, create_signal, view};
use crate::styled::{ContextMenu, Dialog};

#[test]
fn a_dialog_opened_from_a_context_menu_stays_open_when_the_menu_closes() {
    let region = NodeRef::new();
    let surface = NodeRef::new();
    let dismissed = Rc::new(Cell::new(0));
    let (document, [_]) = toolbar_of({
        let region = region.clone();
        let surface = surface.clone();
        let reports = dismissed.clone();
        move || {
            let (open, set_open) = create_signal(false);
            let items = view! {
                <unstyled::MenuItem label="Inspect" />
            };
            [view! {
                <List spacing=0.0>
                    <ContextMenu items on_select={move |_path| set_open.set(true)}>
                        <MenuRegion @node_ref=&region />
                    </ContextMenu>
                    <Dialog
                        open={open}
                        title="Inspect"
                        on_dismiss={move || reports.set(reports.get() + 1)}
                    >
                        <Text
                            string="Details"
                            font_size=14.0
                            color=Color32::WHITE
                            @node_ref={&surface}
                        />
                    </Dialog>
                </List>
            }]
        }
    });
    let region = region.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        dismissed.get(),
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
