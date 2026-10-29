use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_dialog_opened_from_a_menu_row_hangs_from_what_the_menu_was_opened_on() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let row = NodeRef::new();
    let region = NodeRef::new();
    let surface = NodeRef::new();
    let document = build_node(move || {
        let items = view! {
            <unstyled::MenuItem
                label="Rename"
                on_click={move || {
                    if let Some(focused) = focused_node() {
                        row.fill(focused);
                    }
                    open.set(true);
                }}
            />
        };
        view! {
            <List spacing=0.0>
                <Frame height=40.0 />
                <ContextMenu items>
                    <MenuRegion @node_ref=&region />
                </ContextMenu>
                <Dialog
                    open={open}
                    title="Rename"
                    anchor={DialogAnchor::Control(row)}
                    width={Size::Custom(200.0)}
                    on_dismiss={move || open.set(false)}
                >
                    <Frame @node_ref=&surface height=40.0 />
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let held = harness.rect(region.get());
    press_secondary(&mut harness, held.center());
    let label = text_within(harness.document(), harness.document().root.expect("a root"), "Rename")
        .expect("the menu shows its row");
    let at = harness.center(label);
    harness.frame(vec![Input::PointerMoved(at)]);
    harness.click(at);
    harness.settle();

    assert!(open.get_untracked(), "the row opened the dialog");
    let body = harness.rect(surface.get());
    assert!(
        body.top() > held.bottom() && (body.center().x - held.center().x).abs() < 2.0,
        "with the menu closed, the card hangs from the region the menu was opened on: {body:?} under {held:?}"
    );
}
