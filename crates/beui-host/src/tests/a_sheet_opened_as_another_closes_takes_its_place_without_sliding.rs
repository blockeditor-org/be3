use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_sheet_opened_as_another_closes_takes_its_place_without_sliding() {
    let _root = crate::reactive::RootScope::new();
    let (first_content, second_content) = (NodeRef::new(), NodeRef::new());
    let first = create_signal(false);
    let second = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <ModalSheet open={first} fit=true on_close={move || first.set(false)}>
                    <Frame @node_ref=&first_content height=120.0 />
                </ModalSheet>
                <ModalSheet open={second} fit=true on_close={move || second.set(false)}>
                    <Frame @node_ref=&second_content height=300.0 />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    let top = |harness: &Harness, content: NodeRef| {
        harness
            .document()
            .node_rect(content.get())
            .map(|rect| rect.top())
    };
    first.set(true);
    harness.settle();
    assert_eq!(top(&harness, first_content), Some(480.0));

    first.set(false);
    second.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        top(&harness, first_content),
        None,
        "the closing sheet gives way at once rather than sliding down"
    );
    let mut was = top(&harness, second_content).expect("the next sheet is laid out at once");
    assert!(
        was <= 480.5 && was > 320.0,
        "the next sheet starts where the closing one stood and grows from there rather than sliding up from below, its content at {was}"
    );
    let mut frames = 0;
    while frames < 120 && harness.frame(Vec::new()).repaint {
        frames += 1;
        let now = top(&harness, second_content).expect("the next sheet stays on screen");
        assert!(
            now <= was + 0.01,
            "and only grows to its own height, never dropping: {now} after {was}"
        );
        was = now;
    }
    harness.settle();
    assert_eq!(
        top(&harness, second_content),
        Some(300.0),
        "it rests at its own height"
    );
}
