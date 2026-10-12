use super::*;
use crate::reactive::{
    Direction, ItemSize, LayoutBuilder, List, NodeRef, ReadSignal, create_effect, view,
};

#[test]
fn a_layout_builder_in_a_wrapping_row_builds_only_for_the_width_it_is_laid_out_at() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let builder = NodeRef::new();
    let document = build({
        let (seen, builder) = (Rc::clone(&seen), builder.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal wrap=true spacing=10.0>
                    <Frame width=40.0 height=20.0 />
                    <LayoutBuilder @sizing=ItemSize::Percent(100.0) @node_ref=&builder>
                        {move |size: ReadSignal<Vec2>| {
                            let seen = Rc::clone(&seen);
                            create_effect(move || seen.borrow_mut().push(size.get().x));
                            view! { <Frame height=20.0 /> }
                        }}
                    </LayoutBuilder>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(150.0, 300.0));
    harness.frame(Vec::new());
    let width = harness.rect(builder.get()).width();
    assert!(width > 0.0);
    assert_eq!(
        *seen.borrow(),
        vec![0.0, width],
        "the builder hears the width it is laid out at, never a width it was only measured at"
    );
}
