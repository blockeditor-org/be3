use super::*;
use crate::reactive::*;

#[test]
fn a_closed_sheet_gives_its_space_back() {
    let _root = crate::reactive::RootScope::new();
    let filler = NodeRef::new();
    let open = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&filler @sizing=ItemSize::Percent(100.0) />
                <styled::Sheet extent=600.0 open={open} on_close={|| ()}>
                    <Frame height=1000.0 />
                </styled::Sheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    assert_eq!(
        harness.rect(filler.get()).height(),
        300.0,
        "an open sheet takes its share"
    );

    open.set(false);
    harness.settle();
    assert_eq!(
        harness.rect(filler.get()).height(),
        600.0,
        "once it has slid away, a closed sheet takes no space"
    );
}
