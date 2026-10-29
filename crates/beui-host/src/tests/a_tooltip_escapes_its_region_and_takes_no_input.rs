use std::time::Duration;

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;
use crate::unstyled::{Tooltip, TooltipHandle};

#[test]
fn a_tooltip_escapes_its_region_and_takes_no_input() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=60.0 />
                <Tooltip
                    label="A label too long to fit under a button this low"
                    delay={Duration::ZERO}
                    content={move |handle: TooltipHandle| {
                        view! {
                            <Frame width=240.0 height=40.0>
                                <Text @test_id="tip">{handle.label}</Text>
                            </Frame>
                        }
                    }}
                >
                    <Button @node_ref=&button>
                        <Text>"add"</Text>
                    </Button>
                </Tooltip>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(160.0, 90.0));
    give_layer_space(&mut harness);
    let center = harness.center(button.get());
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.frame(Vec::new());

    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1, "the tooltip escapes");
    let layer = &layers[0];
    let below = LAYER_SPACE.to_host.rect(harness.rect(button.get())).bottom();
    assert!(
        layer.rect.top() >= below - 1.0,
        "with room below in the layer space it stays under its button rather than flipping above: {:?}",
        layer.rect
    );
    assert!(
        !layer.dismiss_on_outside_press
            && !layer.keyboard
            && !layer.blocks_owner
            && layer.floating.is_empty()
            && !layer.painted.is_empty(),
        "a tooltip takes no input: {layer:?}"
    );
}
