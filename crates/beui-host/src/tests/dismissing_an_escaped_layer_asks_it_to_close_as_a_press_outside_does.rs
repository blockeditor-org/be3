use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn dismissing_an_escaped_layer_asks_it_to_close_as_a_press_outside_does() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let heard = create_signal(Vec::<Dismiss>::new());
    let document = build_node(move || {
        view! {
            <Overlay
                anchor=OverlayAnchor::Point(pos2(20.0, 20.0))
                placement=Placement::BelowStart
                open={open}
                on_dismiss={move |why: Dismiss| {
                    heard.update(|heard| heard.push(why));
                    open.set(false);
                }}
            >
                <Frame width=80.0 height=60.0 />
            </Overlay>
        }
    });
    let mut harness = Harness::new(document);
    give_layer_space(&mut harness);
    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1);

    harness.context().dismiss_layer(layers[0].id);
    harness.frame(Vec::new());
    assert_eq!(heard.get_untracked(), vec![Dismiss::OutsidePress]);
    assert!(
        harness.document().escaped_layers().is_empty(),
        "the owner closed it"
    );

    harness.context().dismiss_layer(layers[0].id);
    harness.frame(Vec::new());
    assert_eq!(
        heard.get_untracked().len(),
        1,
        "a layer that has closed is not asked again"
    );
}
