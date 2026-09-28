use super::*;
use crate::geometry::vec2;
use crate::reactive::{VirtualList, build, create_signal, view, with_reactive_scope};
use crate::unstyled::Scroll;

const ROW: f32 = 40.0;

#[test]
fn a_row_that_leaves_a_virtual_list_moves_the_rows_below_it_by_a_copy() {
    let (keys, set_keys) = create_signal((0..30).collect::<Vec<usize>>());
    let document = build(move || {
        view! {
            <Frame color=Color32::BLACK radius=0>
                <List spacing=0.0>
                    <Frame height=200.0>
                        <Scroll @test_id="scroll">
                            <VirtualList keys item_size=ROW>
                                {|row: usize| view! {
                                    <Frame height=ROW>
                                        <Text string={format!("row {row}")} />
                                    </Frame>
                                }}
                            </VirtualList>
                        </Scroll>
                    </Frame>
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let viewport = harness.rect(harness.find("scroll"));

    with_reactive_scope(harness.document_mut(), move || {
        set_keys.update(|keys| {
            keys.retain(|key| *key != 1);
        })
    });
    let output = harness.frame(Vec::new());
    let moved = output
        .moved()
        .expect("the rows below the one that left move up by a copy");
    assert_eq!(moved.by, vec2(0.0, -ROW));
    assert_eq!(moved.to().top(), ROW, "the copy starts where the row that left was");
    assert!(moved.to().bottom() > viewport.bottom() - 2.0 * ROW);
    assert!(moved.to().bottom() <= viewport.bottom() - ROW);
    let damaged: f32 = output.damage.rects().iter().map(|rect| area(*rect)).sum();
    assert!(
        damaged <= area(moved.to()) / 2.0,
        "only the strip the rows left behind is painted: {:?}",
        output.damage.rects()
    );
    assert!(
        output
            .damage
            .rects()
            .iter()
            .all(|rect| rect.top() >= moved.to().bottom() && rect.bottom() <= viewport.bottom()),
        "only the strip below the copy is painted: {:?}",
        output.damage.rects()
    );
}

fn area(rect: Rect) -> f32 {
    match rect.is_positive() {
        true => rect.width() * rect.height(),
        false => 0.0,
    }
}
