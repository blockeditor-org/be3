use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::*;

#[test]
fn a_scroll_in_a_dialog_copies_what_it_showed_over_the_page_beneath() {
    let _root = crate::reactive::RootScope::new();
    let first = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ForEach keys={(0..40).collect::<Vec<usize>>()}>|line: usize|
                    <Text>{format!("the page beneath the dialog, line {line}")}</Text>
                </ForEach>
                <Dialog open=true title="Add block" width={Size::Custom(300.0)} on_dismiss={|| {}}>
                    <Frame height=200.0>
                        <Scroll>
                            <List spacing=8.0>
                                <Frame @node_ref=&first height=120.0>
                                    <Text>"first"</Text>
                                </Frame>
                                <Frame height=120.0>
                                    <Text>"second"</Text>
                                </Frame>
                                <Frame height=120.0>
                                    <Text>"third"</Text>
                                </Frame>
                            </List>
                        </Scroll>
                    </Frame>
                </Dialog>
            </List>
        }
    });
    let first = first.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(vec![Input::PointerMoved(harness.center(first))]);

    let output = harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(Vec2::new(
        0.0, -40.0,
    )))]);
    let moved = output
        .main()
        .moved()
        .expect("the dialog's panel hides the page, so the scroll copies what it showed");
    assert_eq!(moved.by, Vec2::new(0.0, -40.0));
}
