use super::*;
use crate::reactive::{
    Direction, ItemSize, LayoutBuilder, List, NodeRef, ReadSignal, create_effect, view,
};

#[test]
fn a_layout_builder_in_a_wrapping_row_builds_only_for_the_width_it_is_laid_out_at() {
    let _root = crate::reactive::RootScope::new();
    let (seen, set_seen) = create_signal(Vec::new());
    let builder = NodeRef::new();
    let document = build({
        move || {
            view! {
                <List direction=Direction::Horizontal wrap=true spacing=10.0>
                    <Frame width=40.0 height=20.0 />
                    <LayoutBuilder @sizing=ItemSize::Percent(100.0) @node_ref=&builder>
                        {move |size: ReadSignal<Vec2>| {
                            create_effect(move || set_seen.update(|seen| seen.push(size.get().x)));
                            view! {
                                <Frame height=20.0 />
                            }
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
        seen.get_untracked(),
        vec![0.0, width],
        "the builder hears the width it is laid out at, never a width it was only measured at"
    );
}
