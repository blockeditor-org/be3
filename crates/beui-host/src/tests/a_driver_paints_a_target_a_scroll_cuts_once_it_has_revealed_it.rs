use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_driver_paints_a_target_a_scroll_cuts_once_it_has_revealed_it() {
    let document = build_node(|| {
        view! {
            <Frame height=100.0>
                <Scroll>
                    <List spacing=0.0>
                        <ForEach keys={indices(12)}>|index: usize|
                            <Frame
                                height=40.0
                                fill=Color32::WHITE
                                @test_id={format!("row.{index}")}
                            />
                        </ForEach>
                    </List>
                </Scroll>
            </Frame>
        }
    });
    let mut driven = stepped(crate::native::testing::Showing(document));
    let height = |place: &str| -> f32 {
        let size = place.split(" size ").nth(1).expect("a place has a size");
        let (_, height) = size.trim().split_once('x').expect("a size is WxH");
        height.parse().expect("a height is a number")
    };

    let cut = driven
        .ask(&["shown", "#row.2"])
        .expect("row 2 shows in part");
    assert_eq!(height(&cut), 20.0, "{cut}");
    let painting = driven.painting_of("#row.2");
    let row = driven.ask(&["shown", "#row.2"]).expect("row 2 shows");
    assert_eq!(
        height(&row),
        40.0,
        "the paint revealed the row first: {row}"
    );
    assert_eq!(painting.frame.size[1] as f32, height(&row));

    driven
        .ask(&["shown", "#row.9"])
        .expect_err("row 9 is scrolled out of view");
    let painting = driven.painting_of("#row.9");
    let row = driven.ask(&["shown", "#row.9"]).expect("row 9 shows");
    assert_eq!(height(&row), 40.0, "{row}");
    assert_eq!(painting.frame.size[1] as f32, height(&row));
}
