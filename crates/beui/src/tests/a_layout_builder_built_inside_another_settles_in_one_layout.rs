use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_layout_builder_built_inside_another_settles_in_one_layout() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Frame width=300.0 height=200.0>
                <LayoutBuilder>|size: ReadSignal<Vec2>| {
                    let wide = create_memo(move || size.get().x > 100.0);
                    view! {
                        <Frame height=40.0>
                            <List spacing=0.0>
                                <Show condition={wide}>
                                    <LayoutBuilder>|inner: ReadSignal<Vec2>| {
                                        create_effect(move || seen.update(|seen| seen.push(inner.get().x)));
                                        view! {
                                            <Frame height=20.0 />
                                        }
                                    }</LayoutBuilder>
                                </Show>
                            </List>
                        </Frame>
                    }
                }</LayoutBuilder>
            </Frame>
        }
    });
    let mut harness = harness(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        1,
        "the inner builder is reached and built later in the same walk"
    );
    assert_eq!(seen.get_untracked().last(), Some(&300.0));
}
