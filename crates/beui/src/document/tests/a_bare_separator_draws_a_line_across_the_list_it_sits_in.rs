use super::*;
use crate::reactive::{Direction, Frame, NodeRef, Row, build, view};
use crate::styled::Separator;
use crate::styled::theme::SEPARATOR_THICKNESS;

const BAR_HEIGHT: f32 = 20.0;
const ITEM_WIDTH: f32 = 40.0;

#[test]
fn a_bare_separator_draws_a_line_across_the_list_it_sits_in() {
    let (flat, upright) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (flat, upright) = (flat.clone(), upright.clone());
        move || {
            view! {
                <Column spacing=0.0>
                    <Separator @node_ref=&flat />
                    <Row spacing=0.0>
                        <Frame width=ITEM_WIDTH height=BAR_HEIGHT />
                        <Separator direction=Direction::Vertical @node_ref=&upright />
                    </Row>
                </Column>
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let flat = harness.rect(flat.get());
    assert_eq!(
        flat.height(),
        SEPARATOR_THICKNESS,
        "a separator running horizontally must measure to its thickness"
    );
    assert_eq!(
        flat.width(),
        WIDE_VIEWPORT.x,
        "a separator running horizontally must stretch across its column"
    );

    let upright = harness.rect(upright.get());
    assert_eq!(
        upright.width(),
        SEPARATOR_THICKNESS,
        "a separator running vertically must measure to its thickness"
    );
    assert_eq!(
        upright.height(),
        BAR_HEIGHT,
        "a separator running vertically must stretch down its row"
    );
}
