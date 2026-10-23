use super::*;
use crate::reactive::*;

#[test]
fn a_canvas_item_resized_while_its_content_changes_is_laid_out_once_where_it_lands() {
    let _root = crate::reactive::RootScope::new();
    let item = NodeRef::new();
    let size = create_signal(vec2(200.0, 100.0));
    let inset = create_signal(0.0f32);
    let mut document = build_node(move || {
        let width = create_memo(move || size.get().x);
        let height = create_memo(move || size.get().y);
        view! {
            <Frame width=600.0 height=400.0>
                <Canvas>
                    <CanvasItem x=10.0 y=10.0 width height>
                        <Frame padding={create_memo(move || Sides::horizontal(inset.get()))}>
                            <Frame @node_ref=&item />
                        </Frame>
                    </CanvasItem>
                </Canvas>
            </Frame>
        }
    });
    let item = item.get();
    let (layouts, _) = counted(&mut document, item);
    let mut harness = Harness::new(document);
    harness.settle();
    let settled = layouts.get_untracked();

    size.set(vec2(300.0, 150.0));
    inset.set(4.0);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(14.0, 10.0), vec2(292.0, 150.0)),
        "the content is laid out at the size the item was given"
    );
    assert_eq!(
        layouts.get_untracked(),
        settled + 1,
        "and only there, not first at the size the item had before"
    );
}
