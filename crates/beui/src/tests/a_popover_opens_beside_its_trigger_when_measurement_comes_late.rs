use super::*;
use crate::primitives::{Frame, List, Text};
use crate::reactive::*;
use crate::styled::Popover;
use crate::unstyled::PopoverHandle;

#[test]
fn a_popover_opens_beside_its_trigger_when_measurement_comes_late() {
    let (prompt, late) = prompt_and_late(|| {
        let _root = crate::reactive::RootScope::new();
        let popover = NodeRef::new();
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <Frame height=120.0 />
                    <Popover @node_ref=&popover label="Filters">|_: PopoverHandle|
                        <Text>"inside"</Text>
                    </Popover>
                </List>
            }
        });
        let mut harness = Harness::new(document);
        harness.settle();
        let trigger = harness
            .document()
            .api::<unstyled::PopoverApi>(popover.get())
            .trigger();
        harness.click(harness.center(trigger));
        harness.settle();
        text_rects(&harness)
    });
    assert!(
        prompt
            .iter()
            .any(|(text, rect)| text == "inside" && rect.is_some()),
        "the popover opened: {prompt:?}"
    );
    assert_eq!(
        late, prompt,
        "late measurement settles where prompt measurement does"
    );
}
