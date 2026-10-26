use super::*;
use crate::base::ScrollPosition;
use crate::primitives::Frame;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

const BAR_HEIGHT: f32 = 8.0;
const ROWS: usize = 100;
const ROW_HEIGHT: f32 = 20.0;

#[test]
fn a_scrollbar_sizes_its_thumb_from_the_scroll_beside_it() {
    let _root = crate::reactive::RootScope::new();
    let thumb = NodeRef::new();
    let document = build_node(move || {
        let position = create_signal(ScrollPosition::ZERO);
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) on_change={position}>
                    <VirtualList
                        keys={indices(ROWS)}
                        item_size=ROW_HEIGHT
                    >|index: ReadSignal<usize>|
                        <Frame height=ROW_HEIGHT>
                            <Text color=Color32::WHITE>
                                {create_memo(move || format!("Row {}", index.get()))}
                            </Text>
                        </Frame>
                    </VirtualList>
                </Offset>
                <Scrollbar @sizing=ItemSize::Fixed(BAR_HEIGHT) @node_ref=&thumb position />
            </List>
        }
    });

    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let content = ROWS as f32 * ROW_HEIGHT;
    let viewport = VIEWPORT.y - BAR_HEIGHT;
    let track = harness.rect(track_of(&harness, thumb.get()));
    let painted = painted_thumb(&output, track).height();
    let track = track.height();
    let wanted = track * (viewport / content);
    assert!(
        painted < track / 2.0,
        "the thumb filled the track: {painted} of {track}"
    );
    assert_eq!(painted, painted.round(), "the thumb missed the pixel grid");
    assert!(
        (painted - wanted).abs() <= 1.0,
        "thumb {painted} wanted {wanted}"
    );
}

fn track_of(harness: &Harness, bar: NodeId) -> NodeId {
    let interactive = harness.document().children(bar)[0];
    let inset = harness.document().children(interactive)[0];
    harness.document().children(inset)[0]
}
