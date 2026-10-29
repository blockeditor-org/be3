use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_container_in_a_zoomed_canvas_item_answers_with_its_unzoomed_size() {
    let _root = crate::reactive::RootScope::new();
    let zoom = create_signal(2.0f32);
    let seen = create_signal(Vec2::ZERO);
    let document = build_node(move || {
        let view = create_memo(move || Some(CanvasView::new(Pos2::ZERO, zoom.get())));
        view! {
            <Canvas view>
                <CanvasItem x=10.0 y=10.0 width=200.0 height=100.0 scales=true>
                    <unstyled::Container>|size: ReadSignal<Vec2>| {
                        create_effect(move || seen.set(size.get()));
                        view! {
                            <Frame fill=Color32::WHITE />
                        }
                    }</unstyled::Container>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        seen.get_untracked(),
        vec2(200.0, 100.0),
        "a container is as wide as its content is laid out, not as wide as the zoom draws it"
    );

    zoom.set(3.0);
    harness.frame(Vec::new());
    assert_eq!(
        seen.get_untracked(),
        vec2(200.0, 100.0),
        "zooming the canvas leaves the size a container answers with alone"
    );
}
