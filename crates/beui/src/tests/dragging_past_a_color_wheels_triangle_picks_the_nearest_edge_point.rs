use super::*;
use crate::color::Hsva;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn dragging_past_a_color_wheels_triangle_picks_the_nearest_edge_point() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let wheel = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ColorWheel
                    @node_ref=&wheel
                    value={Binding::new(Hsva::new(0.0, 1.0, 1.0, 1.0).to_color(), move |next| changes.update(|changes| changes.push(next)))}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let rect = harness.rect(wheel.get());
    let centre = pos2(
        rect.left() + WHEEL_SIZE / 2.0,
        rect.top() + WHEEL_SIZE / 2.0,
    );

    harness.drag(centre, pos2(centre.x - WHEEL_SIZE * 0.35, centre.y));
    harness.frame(Vec::new());

    let changes = changes.get_untracked();
    assert_eq!(changes.len(), 1, "a drag reports once, when it ends");
    let picked = Hsva::from_color(changes[0]);
    assert!(picked.saturation < 0.02, "{picked:?}");
    assert!((picked.value - 0.5).abs() < 0.02, "{picked:?}");
}
