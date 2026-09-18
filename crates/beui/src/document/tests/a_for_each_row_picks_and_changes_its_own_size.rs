use super::*;
use crate::reactive::{
    Button, Column, ForEach, Frame, ItemSize, NodeRef, Text, build, create_memo, create_signal,
    view,
};

const SHORT_HEIGHT: f32 = 30.0;
const TALL_HEIGHT: f32 = 80.0;

#[test]
fn a_for_each_row_picks_and_changes_its_own_size() {
    let (toggle, header, body) = (NodeRef::new(), NodeRef::new(), NodeRef::new());

    let document = build({
        let (toggle, header, body) = (toggle.clone(), header.clone(), body.clone());
        move || {
            let (tall, set_tall) = create_signal(false);
            let sizing = create_memo(move || match tall.get() {
                true => ItemSize::Fixed(TALL_HEIGHT),
                false => ItemSize::Fixed(SHORT_HEIGHT),
            });
            view! {
                <Column spacing=0.0>
                    <Button
                        @node_ref=&toggle
                        on_click={move || set_tall.update(|tall| *tall = !*tall)}
                    >
                        <Text string="toggle" />
                    </Button>
                    <ForEach keys={vec![0, 1]}>
                        {move |index: usize| {
                            let (header, body) = (header.clone(), body.clone());
                            match index {
                                0 => view! {
                                    <Frame @sizing={sizing.clone()} @node_ref=&header />
                                },
                                _ => {
                                    view! {
                                        <Frame @sizing=ItemSize::Percent(100.0) @node_ref=&body />
                                    }
                                }
                            }
                        }}
                    </ForEach>
                </Column>
            }
        }
    });

    let (toggle, header, body) = (toggle.get(), header.get(), body.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let rows = WIDE_VIEWPORT.y - harness.rect(toggle).height();
    assert_eq!(
        harness.rect(header).height(),
        SHORT_HEIGHT,
        "each row of a `ForEach` picks its own size rather than sharing one"
    );
    assert_eq!(harness.rect(body).height(), rows - SHORT_HEIGHT);

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(header).height(),
        TALL_HEIGHT,
        "a reactive size on a row must reach the list the row was built into"
    );
    assert_eq!(harness.rect(body).height(), rows - TALL_HEIGHT);
}
