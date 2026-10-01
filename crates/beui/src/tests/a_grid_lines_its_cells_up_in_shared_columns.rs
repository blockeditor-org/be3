use super::*;
use crate::reactive::{Frame, Grid, GridCell, NodeRef, Track, view};

#[test]
fn a_grid_lines_its_cells_up_in_shared_columns() {
    let cells: Vec<NodeRef> = (0..5).map(|_| NodeRef::new()).collect();
    let document = build({
        let [label, value, longer, other, wide] =
            <[NodeRef; 5]>::try_from(cells.clone()).unwrap_or_else(|_| unreachable!());
        move || {
            view! {
                <Grid
                    columns={vec![Track::Intrinsic, Track::Fraction(1.0), Track::Fixed(30.0)]}
                    column_spacing=10.0
                    row_spacing=4.0
                >
                    <Frame @node_ref=&label width=40.0 height=10.0 />
                    <Frame @node_ref=&value height=20.0 />
                    <Frame width=5.0 height=5.0 />
                    <Frame @node_ref=&longer width=70.0 height=10.0 />
                    <Frame @node_ref=&other height=10.0 />
                    <Frame width=5.0 height=5.0 />
                    <GridCell span=3>
                        <Frame @node_ref=&wide height=10.0 />
                    </GridCell>
                </Grid>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 200.0));
    harness.frame(Vec::new());
    let rect = |index: usize| harness.rect(cells[index].get());

    assert_eq!(
        rect(0),
        Rect::from_min_size(pos2(0.0, 0.0), vec2(70.0, 20.0))
    );
    assert_eq!(
        rect(1),
        Rect::from_min_size(pos2(80.0, 0.0), vec2(180.0, 20.0))
    );
    assert_eq!(
        rect(2),
        Rect::from_min_size(pos2(0.0, 24.0), vec2(70.0, 10.0))
    );
    assert_eq!(
        rect(3),
        Rect::from_min_size(pos2(80.0, 24.0), vec2(180.0, 10.0))
    );
    assert_eq!(
        rect(4),
        Rect::from_min_size(pos2(0.0, 38.0), vec2(300.0, 10.0))
    );
}
