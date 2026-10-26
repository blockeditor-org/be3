use super::*;
use crate::geometry::Rect;
use crate::painter::Shape;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

const SIBLING: Color32 = Color32::from_rgb(200, 40, 40);

#[test]
fn a_dialogs_scrim_dims_what_comes_after_it_in_the_tree() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog open={open} title="Discard changes?" on_dismiss={|| ()}>
                    <Frame width=10.0 height=10.0 />
                </Dialog>
                <Frame width=50.0 height=50.0 fill=SIBLING />
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    open.set(true);
    let output = harness.frame(Vec::new());
    let shapes = output.main().shapes();
    let viewport = Rect::from_min_size(Pos2::ZERO, VIEWPORT);
    let position = |wanted: &dyn Fn(Rect, Color32) -> bool| {
        shapes.iter().position(|shape| match shape {
            Shape::Rect { rect, color, .. } => wanted(*rect, *color),
            _ => false,
        })
    };
    let after = position(&|_, color| color == SIBLING).expect("the frame is painted");
    let scrim = position(&|rect, color| rect == viewport && color.alpha() > 0)
        .expect("the scrim is painted");
    assert!(
        after < scrim,
        "the scrim paints over the frame after the dialog"
    );
}
