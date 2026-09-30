use super::*;
use crate::geometry::vec2;
use crate::reactive::{Direction, ForEach, List, Text, build, view};
use crate::unstyled::Scroll;

const ROWS: usize = 30;
const LABELS: usize = 6;

#[test]
fn scrolling_lays_out_and_paints_the_rows_it_exposes_rather_than_every_row_that_moved() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Frame height=200.0>
                    <Scroll @test_id="scroll">
                        <ForEach keys={(0..ROWS).collect::<Vec<_>>()}>
                            {|row: usize| view! {
                                <Frame height=40.0 color=Color32::WHITE radius=0>
                                    <List direction=Direction::Horizontal spacing=4.0>
                                        <ForEach keys={(0..LABELS).collect::<Vec<_>>()}>
                                            {move |label: usize| view! {
                                                <Text string={format!("{row}.{label}")} />
                                            }}
                                        </ForEach>
                                    </List>
                                </Frame>
                            }}
                        </ForEach>
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let scroll = harness.find("scroll");
    harness.frame(vec![Event::PointerMoved(harness.rect(scroll).center())]);

    harness.frame(vec![Event::Scroll(vec2(0.0, -40.0))]);
    assert_eq!(harness.document().scroll_offset(scroll), 40.0);
    let work = harness.document().performance().latest.work;
    let row = LABELS + 2;
    assert!(
        work.placed < row * 3,
        "a scroll moves its rows without laying them out again, so only the row it exposes \
         and the scroll itself are laid out: {} nodes were",
        work.placed
    );
    assert!(
        work.painted_nodes < row * 3,
        "a scroll moves its rows without painting them again, so only the row it exposes \
         and the scroll itself are painted: {} nodes were",
        work.painted_nodes
    );
}
