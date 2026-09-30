use super::*;
use crate::reactive::{ForEach, NodeRef, Text};
use crate::styled::{Dialog, Scroll};

#[test]
fn a_scroll_in_a_dialog_copies_what_it_showed_over_the_page_beneath() {
    let first = NodeRef::new();
    let document = build({
        let first = first.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <ForEach keys={(0..40).collect::<Vec<usize>>()}>
                        {|line: usize| view! {
                            <Text string={format!("the page beneath the dialog, line {line}")} />
                        }}
                    </ForEach>
                    <Dialog open=true title="Add block" width=300.0 on_dismiss={|| {}}>
                        <Frame height=200.0>
                            <Scroll>
                                <List spacing=8.0>
                                    <Frame @node_ref=&first height=120.0>
                                        <Text string="first" />
                                    </Frame>
                                    <Frame height=120.0>
                                        <Text string="second" />
                                    </Frame>
                                    <Frame height=120.0>
                                        <Text string="third" />
                                    </Frame>
                                </List>
                            </Scroll>
                        </Frame>
                    </Dialog>
                </List>
            }
        }
    });
    let first = first.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(vec![Event::PointerMoved(harness.center(first))]);

    let output = harness.frame(vec![Event::Scroll(Vec2::new(0.0, -40.0))]);
    let moved = output
        .moved()
        .expect("the dialog's panel hides the page, so the scroll copies what it showed");
    assert_eq!(moved.by, Vec2::new(0.0, -40.0));
}
