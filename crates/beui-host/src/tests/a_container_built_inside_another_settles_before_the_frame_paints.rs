use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Container;

#[test]
fn a_container_built_inside_another_settles_before_the_frame_paints() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Frame width=300.0 height=200.0>
                <Container>|size: ReadSignal<Vec2>| {
                    let wide = create_memo(move || size.get().x > 100.0);
                    view! {
                        <Frame height=40.0>
                            <List spacing=0.0>
                                <Show condition={wide}>
                                    <Container>|inner: ReadSignal<Vec2>| {
                                        create_effect(move || seen.update(|seen| seen.push(inner.get().x)));
                                        view! {
                                            <Frame height=20.0 />
                                        }
                                    }</Container>
                                </Show>
                            </List>
                        </Frame>
                    }
                }</Container>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        2,
        "the inner container is built from the outer's measurement and laid out again in the same frame"
    );
    assert_eq!(seen.get_untracked().last(), Some(&300.0));
}
