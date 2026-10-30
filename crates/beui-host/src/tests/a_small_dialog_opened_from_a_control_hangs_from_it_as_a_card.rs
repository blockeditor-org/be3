use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_small_dialog_opened_from_a_control_hangs_from_it_as_a_card() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let control = NodeRef::new();
    let surface = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=40.0 />
                <Button @node_ref=&control label="Resize" on_click={move || open.set(true)} />
                <Dialog
                    open={open}
                    title="Resize"
                    anchor={DialogAnchor::Control(control)}
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
    let button = harness.rect(control.get());
    harness.click(button.center());
    harness.frame(Vec::new());

    let body = harness.rect(surface.get());
    assert!(
        body.top() > button.bottom(),
        "the card opens under the control that opened it: {body:?} under {button:?}"
    );
    assert!(
        (body.center().x - button.center().x).abs() < 2.0,
        "and is centred on it: {body:?} on {button:?}"
    );

    give_layer_space(&mut harness);
    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1, "a card from a control escapes its region");
    let layer = &layers[0];
    assert!(layer.scrims.is_empty(), "a card dims nothing");
    assert!(layer.dismiss_on_outside_press && layer.blocks_owner && layer.keyboard);

    harness.key(Key::Escape, Modifiers::NONE);
    harness.settle();
    assert!(!open.get_untracked(), "Escape closes it");
    assert_eq!(
        harness.document().focused,
        Some(control.get()),
        "and the focus goes back to the control"
    );
}
