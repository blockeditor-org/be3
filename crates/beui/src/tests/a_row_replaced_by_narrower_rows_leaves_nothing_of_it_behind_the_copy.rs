use super::*;
use crate::geometry::{pos2, vec2};
use crate::reactive::{VirtualList, build, create_signal, view, with_reactive_scope};
use crate::unstyled::Scroll;

const ROW: f32 = 40.0;

#[test]
fn a_row_replaced_by_narrower_rows_leaves_nothing_of_it_behind_the_copy() {
    let (keys, set_keys) = create_signal((0..30).collect::<Vec<usize>>());
    let document = build(move || {
        view! {
            <Frame color=Color32::BLACK radius=0>
                <List spacing=0.0>
                    <Frame height=200.0>
                        <Scroll>
                            <VirtualList keys item_size=ROW>
                                {|row: usize| view! {
                                    <Frame height=ROW>
                                        <Text
                                            string={match row {
                                                1 => "a row with a much longer name".to_owned(),
                                                row => format!("{row}"),
                                            }}
                                        />
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

    with_reactive_scope(harness.document_mut(), move || {
        set_keys.update(|keys| {
            keys.splice(1..2, [100, 101]);
        })
    });
    let output = harness.frame(Vec::new());
    let moved = output
        .moved()
        .expect("the rows below the two that joined move down by a copy");
    assert_eq!(moved.by, vec2(0.0, ROW));
    let wide = Rect::from_min_max(pos2(0.0, 2.0 * ROW), pos2(100.0, 2.0 * ROW + 10.0));
    assert!(
        output
            .damage
            .rects()
            .iter()
            .any(|rect| rect.contains_rect(wide)),
        "the copy carries the wide row that left under the narrow row that took its place, so that is painted over: {:?}",
        output.damage.rects()
    );
    assert!(
        output
            .damage
            .rects()
            .iter()
            .all(|rect| rect.bottom() <= 3.0 * ROW),
        "the rows the copy moved are not painted again: {:?}",
        output.damage.rects()
    );
}
