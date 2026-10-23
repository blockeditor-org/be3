use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_canvas_reveals_its_focused_text_when_its_view_shrinks() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(Pos2::ZERO, 1.0)));
    let revealed = create_signal(Vec::<Vec2>::new());
    let input = NodeRef::new();

    let document = build_node(move || {
        view! {
            <Canvas
                view={view}
                on_reveal={move |delta: Vec2| {
                    revealed.update(|revealed| revealed.push(delta));
                    view.update(|view| {
                        if let Some(view) = view {
                            view.origin += delta;
                        }
                    });
                }}
            >
                <CanvasItem x=40.0 y=500.0 width=120.0 height=30.0>
                    <TextInput @node_ref={&input} value="" />
                </CanvasItem>
            </Canvas>
        }
    });

    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    harness.click(harness.center(input.get()));
    harness.frame(Vec::new());
    assert!(
        revealed.get_untracked().is_empty(),
        "text already in view is not moved when it takes the focus"
    );

    harness.resize(Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let field = harness.rect(input.get());
    assert!(
        field.min.y >= 0.0 && field.max.y <= 300.0,
        "the field at {field:?} is inside the shrunk view"
    );
    let [delta] = revealed.get_untracked()[..] else {
        panic!("one reveal, got {:?}", revealed.get_untracked());
    };
    assert_eq!(delta.x, 0.0, "the view moves only as far as it must");
    assert!(
        field.max.y > 280.0,
        "the field stays near the view's edge it came from, at {field:?}"
    );
}
