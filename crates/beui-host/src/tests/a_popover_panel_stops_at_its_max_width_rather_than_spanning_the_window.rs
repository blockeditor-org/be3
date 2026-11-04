use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::popover::PANEL_MAX_WIDTH;
use crate::styled::*;
use crate::unstyled::PopoverHandle;

#[test]
fn a_popover_panel_stops_at_its_max_width_rather_than_spanning_the_window() {
    let _root = crate::reactive::RootScope::new();
    let content = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Popover label="Filters">|_: PopoverHandle|
                    <Frame @node_ref={&content}>
                        <Checkbox label="Only starred" checked=false />
                    </Frame>
                </Popover>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let trigger = text_within(
        harness.document(),
        harness.document().root().expect("the popover built a root"),
        "Filters",
    )
    .expect("the trigger shows its label");

    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    let width = harness.rect(content.get()).width();
    assert!(width > 0.0, "the popover opened");
    assert!(
        width <= PANEL_MAX_WIDTH.px(),
        "the panel's content is {width} wide in a {} wide window",
        WIDE_VIEWPORT.x
    );
}
