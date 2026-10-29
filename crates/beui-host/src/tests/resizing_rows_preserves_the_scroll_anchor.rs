use super::*;
use crate::base::frame::Sides;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn resizing_rows_preserves_the_scroll_anchor() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(None);
    let scroll = NodeRef::new();
    let rows: Vec<NodeRef> = (0..100).map(|_| NodeRef::new()).collect();
    let document = build_node({
        let rows = rows.clone();
        move || {
            view! {
                <Scroll @node_ref=&scroll on_change={move |position| reported.set(Some(position))}>
                    <ForEach keys={indices(rows.len())}>|index: usize|
                        <Frame
                            @node_ref={&rows[index]}
                            padding={Sides::symmetric(0.0, 10.0 + (index % 3) as f32)}
                        >
                            <Spacer />
                        </Frame>
                    </ForEach>
                </Scroll>
            }
        }
    });
    let scroll = scroll.get();
    let rows: Vec<NodeOf<FrameNode>> = rows
        .iter()
        .map(|row| kind_of(&document, row.get()))
        .collect();
    let mut harness = Harness::new(document);
    harness.document_mut().set_scroll_offset(scroll, 227.0);
    harness.frame(Vec::new());
    let anchor = rows[10];
    let top = harness.rect(anchor).top();
    assert_eq!(top, -9.0);

    for &row in &rows[..10] {
        harness
            .document_mut()
            .set_frame_padding(row, Sides::symmetric(0.0, 5.0));
    }
    harness.frame(Vec::new());

    assert_eq!(harness.rect(anchor).top(), top);
    assert_eq!(harness.document().scroll_offset(scroll), 109.0);
    assert_eq!(reported.get_untracked().unwrap().offset, 109.0);

    harness.frame(vec![
        Input::PointerMoved(harness.center(scroll)),
        Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, -5.0))),
    ]);
    assert_eq!(harness.rect(anchor).top(), top - 5.0);
    assert_eq!(reported.get_untracked().unwrap().offset, 114.0);

    harness.document_mut().set_scroll_offset(scroll, 0.0);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(rows[0]).top(), 0.0);
    assert_eq!(reported.get_untracked().unwrap().offset, 0.0);
}
