use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_focused_text_input_panned_out_of_a_canvas_keeps_the_keyboard() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(Pos2::ZERO, 1.0)));
    let input = NodeRef::new();

    let document = build_node(move || {
        view! {
            <Canvas view={view}>
                <CanvasItem x=10.0 y=10.0 width=120.0 height=30.0>
                    <TextInput @node_ref={&input} value="" />
                </CanvasItem>
            </Canvas>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(input.get()));
    let asked = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(asked.is_some(), "the focused field asks for the keyboard");
    let focused = harness.document().focused_node();

    view.set(Some(CanvasView::new(pos2(0.0, -5000.0), 1.0)));
    for frame in 0..30 {
        let window = harness.frame(Vec::new()).main().window.clone();
        assert_eq!(
            window.requests.ime.as_ref().map(|area| area.keyboard),
            asked.as_ref().map(|area| area.keyboard),
            "frame {frame}: the keyboard stays asked for while the field is out of view"
        );
        assert_eq!(harness.document().focused_node(), focused);
    }

    view.set(Some(CanvasView::new(Pos2::ZERO, 1.0)));
    let back = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert_eq!(
        back.map(|area| area.keyboard),
        asked.map(|area| area.keyboard)
    );
}
