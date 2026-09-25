use super::*;
use crate::reactive::{NodeRef, Show};
use crate::styled::{Dialog, Scroll};

#[test]
fn a_scroll_in_a_dialog_follows_the_wheel() {
    let first = NodeRef::new();
    let document = build({
        let first = first.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <Dialog open=true title="Add block" width=300.0 on_dismiss={|| {}}>
                        <Frame height=200.0>
                            <List spacing=8.0>
                                <Show condition=true>
                                    <Scroll @sizing=ItemSize::Percent(100.0)>
                                        <List spacing=8.0>
                                            <Frame @node_ref=&first height=120.0 />
                                            <Frame height=120.0 />
                                            <Frame height=120.0 />
                                        </List>
                                    </Scroll>
                                </Show>
                            </List>
                        </Frame>
                    </Dialog>
                </List>
            }
        }
    });
    let first = first.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let top = harness.rect(first).top();

    harness.frame(vec![
        Event::PointerMoved(harness.center(first)),
        Event::Scroll(Vec2::new(0.0, -40.0)),
    ]);
    harness.frame(Vec::new());

    assert!(harness.rect(first).top() < top);
}
