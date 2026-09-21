use super::*;
use crate::base::ScrollPosition;
use crate::reactive::{ItemSize, List, VirtualOffset, create_signal};
use crate::styled::Scrollbar;

const BAR_HEIGHT: f32 = 8.0;
const ROWS: usize = 100;
const ROW_HEIGHT: f32 = 20.0;

#[test]
fn a_scrollbar_sizes_its_thumb_from_the_scroll_beside_it() {
    let thumb = NodeRef::new();
    let document = build({
        let thumb = thumb.clone();
        move || {
            let (position, set_position) = create_signal(ScrollPosition::ZERO);
            view! {
                <List spacing=0.0>
                    <VirtualOffset
                        @sizing=ItemSize::Percent(100.0)
                        count=ROWS
                        item_size=ROW_HEIGHT
                        on_change={move |reported| set_position.set(reported)}
                    >
                        {move |index: usize| view! {
                            <Frame height=ROW_HEIGHT>
                                <Text
                                    string={format!("Row {index}")}
                                    font_size=14.0
                                    color=Color32::WHITE
                                />
                            </Frame>
                        }}
                    </VirtualOffset>
                    <Scrollbar @sizing=ItemSize::Fixed(BAR_HEIGHT) @node_ref=&thumb position />
                </List>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let content = ROWS as f32 * ROW_HEIGHT;
    let viewport = VIEWPORT.y - BAR_HEIGHT;
    let track = harness.rect(thumb.get()).height();
    let painted = thumb_height(&harness, thumb.get());
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

fn thumb_height(harness: &Harness, bar: NodeId) -> f32 {
    let list = harness.document().children(bar)[0];
    let thumb = harness.document().children(list)[1];
    harness.rect(thumb).height()
}
