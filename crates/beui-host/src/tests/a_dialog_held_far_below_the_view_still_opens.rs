use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::sight::CULLING_MARGIN;
use crate::styled::Scroll;
use crate::styled::*;

#[test]
fn a_dialog_held_far_below_the_view_still_opens() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let control = NodeRef::new();
    let surface = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Scroll>
                <List spacing=0.0>
                    <Button @node_ref=&control label="Pick" on_click={move || open.set(true)} />
                    <Frame height={4.0 * (WIDE_VIEWPORT.y + CULLING_MARGIN)} />
                    <Dialog
                        open={open}
                        title="Pick"
                        anchor={DialogAnchor::Control(control)}
                        on_dismiss={move || open.set(false)}
                    >
                        <Frame @node_ref=&surface height=40.0 />
                    </Dialog>
                </List>
            </Scroll>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let button = harness.rect(control.get());
    harness.click(button.center());
    harness.frame(Vec::new());

    let body = harness.rect(surface.get());
    assert!(
        body.top() > button.bottom() && body.bottom() < WIDE_VIEWPORT.y,
        "the card opens under its control, though the dialog is written far below the view: {body:?}"
    );
}
