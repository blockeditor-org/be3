use super::*;
use crate::reactive::{NodeRef, Shift};
use beui_core::display::flatten;
use beui_core::painter::Shape;

#[test]
fn a_shifted_child_in_a_scrolled_view_still_paints() {
    const MARK: Color32 = Color32::from_rgb(200, 30, 90);
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <unstyled::Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <Frame height=500.0 />
                        <Shift by={vec2(10.0, 0.0)}>
                            <Frame height=50.0 color=MARK />
                        </Shift>
                        <Frame height=500.0 />
                    </unstyled::Scroll>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness
        .document_mut()
        .set_scroll_offset(scroll.get(), 300.0);
    let output = harness.frame(Vec::new());
    let marked = flatten(&output.layers)
        .iter()
        .any(|shape| matches!(shape, Shape::Rect { color, .. } if *color == MARK));
    assert!(
        marked,
        "a shifted child scrolled into view paints, though the scroll moved its space"
    );
}
