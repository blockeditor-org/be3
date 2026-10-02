use super::*;
use crate::reactive::{ForEach, Frame, Grid, ItemSize, List, NodeRef, Offset, Spacer, Track, view};

const CELLS: usize = 400;
const CELL_HEIGHT: f32 = 20.0;

#[test]
fn a_grid_in_a_scroll_lays_out_only_the_cells_near_the_view() {
    let cells: Vec<NodeRef> = (0..CELLS).map(|_| NodeRef::new()).collect();
    let insides: Vec<NodeRef> = (0..CELLS).map(|_| NodeRef::new()).collect();
    let document = build({
        let (cells, insides) = (cells.clone(), insides.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0)>
                        <Grid columns={vec![Track::Fraction(1.0); 2]}>
                            <ForEach keys={indices(CELLS)}>
                                {move |index: usize| {
                                    let (cell, inside) = (cells[index].clone(), insides[index].clone());
                                    view! {
                                        <Frame @node_ref=&cell height=CELL_HEIGHT>
                                            <Frame @node_ref=&inside>
                                                <Spacer />
                                            </Frame>
                                        </Frame>
                                    }
                                }}
                            </ForEach>
                        </Grid>
                    </Offset>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let first = (cells[0].get(), insides[0].get());
    let last = (cells[CELLS - 1].get(), insides[CELLS - 1].get());
    assert!(!harness.document().is_culled(first.0));
    assert!(harness.document().node_rect(first.1).is_some());
    assert!(
        harness.document().is_culled(last.0),
        "a cell far below the view is culled"
    );
    assert!(
        harness.document().node_rect(last.0).is_some(),
        "a culled cell keeps its place"
    );
    assert!(
        harness.document().node_rect(last.1).is_none(),
        "nothing inside a culled cell is laid out"
    );
}
