use super::*;

#[test]
fn a_change_inside_a_fixed_size_frame_lays_out_only_that_frame() {
    let list = NodeRef::new();
    let (beside, fixed, inner) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let mut document = build({
        let list = list.clone();
        let (beside, fixed, inner) = (beside.clone(), fixed.clone(), inner.clone());
        move || {
            view! {
                <List @node_ref=&list spacing=0.0>
                    <Frame @node_ref=&beside height=100.0 color=Color32::WHITE radius=0 />
                    <Frame @node_ref=&fixed width=200.0 height=100.0 radius=0>
                        <Frame
                            @node_ref=&inner
                            height=40.0
                            color={Color32::from_gray(40)}
                            radius=0
                        />
                    </Frame>
                </List>
            }
        }
    });
    let (list, beside, fixed, inner) = (list.get(), beside.get(), fixed.get(), inner.get());
    let (list_layouts, _) = counted(&mut document, list);
    let (beside_layouts, _) = counted(&mut document, beside);
    let (fixed_layouts, _) = counted(&mut document, fixed);
    let (inner_layouts, _) = counted(&mut document, inner);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = (
        list_layouts.get(),
        beside_layouts.get(),
        fixed_layouts.get(),
        inner_layouts.get(),
    );
    let placed = harness.rect(fixed);

    harness.document_mut().set_frame_height(inner, Some(80.0));
    harness.frame(Vec::new());

    assert_eq!(
        inner_layouts.get(),
        settled.3 + 1,
        "the frame that changed is laid out again"
    );
    assert_eq!(
        fixed_layouts.get(),
        settled.2 + 1,
        "the fixed-size frame holding it lays out its child again"
    );
    assert_eq!(
        (list_layouts.get(), beside_layouts.get()),
        (settled.0, settled.1),
        "nothing outside the fixed-size frame is laid out again"
    );
    assert_eq!(
        harness.rect(fixed),
        placed,
        "the fixed-size frame keeps the rectangle it was given"
    );
}
