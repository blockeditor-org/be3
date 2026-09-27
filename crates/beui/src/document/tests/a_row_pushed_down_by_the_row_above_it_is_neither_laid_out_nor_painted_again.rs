use super::*;
use crate::reactive::{
    Direction, ForEach, List, Text, build, create_signal, view, with_reactive_scope,
};

const ROWS: usize = 12;
const LABELS: usize = 6;

#[test]
fn a_row_pushed_down_by_the_row_above_it_is_neither_laid_out_nor_painted_again() {
    let (height, set_height) = create_signal(40.0);
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame height={height} color=Color32::WHITE radius=0 />
                <ForEach keys={(0..ROWS).collect::<Vec<_>>()}>
                    {|row: usize| view! {
                        <Frame height=30.0 color=Color32::WHITE radius=0>
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
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), move || set_height.set(70.0));
    harness.frame(Vec::new());
    let work = harness.document().performance().latest.work;
    assert!(
        work.placed < LABELS,
        "the rows below the one that grew only move, so each keeps its layout: {} nodes were \
         laid out",
        work.placed
    );
    assert!(
        work.painted_nodes < LABELS,
        "the rows below the one that grew only move, so each keeps its painting: {} nodes were \
         painted",
        work.painted_nodes
    );
}
