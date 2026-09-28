use super::*;
use crate::reactive::{view, with_reactive_scope};

const GIVEN_WIDTH: f32 = 80.0;
const CHILD_WIDTH: f32 = 20.0;
const CHILD_HEIGHT: f32 = 10.0;

#[test]
fn a_frame_width_bound_to_a_signal_measures_intrinsically_once_it_clears() {
    let outer = NodeRef::new();
    let (width, set_width) = create_signal(Some(GIVEN_WIDTH));
    let document = build({
        let outer = outer.clone();
        move || {
            view! {
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame @node_ref=&outer width={width}>
                        <Frame width=CHILD_WIDTH height=CHILD_HEIGHT />
                    </Frame>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            }
        }
    });
    let outer = outer.get();
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(outer).width(), GIVEN_WIDTH);

    with_reactive_scope(harness.document_mut(), move || set_width.set(None));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(outer).width(),
        CHILD_WIDTH,
        "clearing the width signal must hand the frame back to its intrinsic measurement"
    );
}
