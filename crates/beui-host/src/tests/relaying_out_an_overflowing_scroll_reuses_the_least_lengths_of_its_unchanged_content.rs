use super::*;
use crate::base::Direction;
use crate::native::with_reactive_scope;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const LABELS: usize = 40;

#[test]
fn relaying_out_an_overflowing_scroll_reuses_the_least_lengths_of_its_unchanged_content() {
    let _root = crate::reactive::RootScope::new();
    let width = create_signal(300.0f32);
    let document = build_node(move || {
        view! {
            <List spacing=0.0 align=Align::Start>
                <Frame width={create_memo(move || Extent::px(width.get()))} height=40.0>
                    <Scroll direction=Direction::Horizontal>
                        <ForEach keys={(0..LABELS).collect::<Vec<_>>()}>|label: usize|
                            <Text
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                overflow=Overflow::Ellipsis
                            >{format!("Label number {label}")}</Text>
                        </ForEach>
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let first = harness.document().performance().latest.work;
    assert!(
        first.least_lengths > LABELS,
        "the first layout of an overflowing scroll asks each label how short it can be: {first:?}"
    );

    for step in 1..=3 {
        with_reactive_scope(harness.document_mut(), move || {
            width.set(300.0 - 20.0 * step as f32)
        });
        harness.frame(Vec::new());
        let work = harness.document().performance().latest.work;
        assert!(
            work.least_lengths < 4 && work.reused_least_lengths > 0,
            "laying the scroll out again at another width reuses its unchanged content's least \
             length instead of walking every label again: {work:?}"
        );
    }
}
