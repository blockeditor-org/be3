use super::*;
use crate::primitives::Frame;
use crate::styled::*;

#[test]
fn a_toast_stack_taller_than_its_area_scrolls_inside_it() {
    let document = build_node(move || {
        let area = NodeRef::new();
        let toasts = (1..=3)
            .map(|id| Toast {
                id,
                title: format!("Toast {id}"),
                message: "word ".repeat(80),
                sticky: true,
                ..Toast::default()
            })
            .collect::<Vec<_>>();
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Toasts anchor={area} toasts={toasts} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let first = output
        .test_id_rect("toast.1")
        .expect("the oldest toast is laid out");
    let newest = output
        .test_id_rect("toast.3")
        .expect("the newest toast shows");
    assert!(
        first.top() < 0.0,
        "the stack is taller than its area, so its start scrolls out of view: {first:?}"
    );
    assert!(
        newest.bottom() <= VIEWPORT.y,
        "and the newest toast, at the bottom edge, stays inside the area: {newest:?}"
    );
}
