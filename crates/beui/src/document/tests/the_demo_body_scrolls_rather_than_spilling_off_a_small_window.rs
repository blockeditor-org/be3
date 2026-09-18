#![allow(dead_code)]

use super::*;

include!("../../../examples/demo.rs");

const WINDOWS: [Vec2; 6] = [
    Vec2::new(360.0, 700.0),
    Vec2::new(500.0, 500.0),
    Vec2::new(721.0, 901.0),
    Vec2::new(900.0, 900.0),
    Vec2::new(1200.0, 500.0),
    Vec2::new(1920.0, 1080.0),
];
const WHEEL_STEP: f32 = 200.0;
const WHEEL_TICKS: usize = 40;
const OVER_A_CARD: Pos2 = Pos2::new(20.0, 200.0);

#[test]
fn the_demo_body_scrolls_rather_than_spilling_off_a_small_window() {
    for window in WINDOWS {
        let mut harness = Harness::sized(DemoApp::new().document, window);
        harness.frame(Vec::new());
        for _ in 0..WHEEL_TICKS {
            harness.scroll(OVER_A_CARD, Vec2::new(0.0, -WHEEL_STEP), Modifiers::NONE);
        }
        harness.frame(Vec::new());
        let root = harness.document.root().expect("the demo built a root");
        let bottom = lowest_edge(harness.document(), root);
        assert!(
            bottom <= window.y,
            "the demo reaches {bottom} in a {window:?} window"
        );
    }
}

fn lowest_edge(document: &Document, id: NodeId) -> f32 {
    let own = document.node_rect(id).map_or(0.0, |rect| rect.bottom());
    document
        .children(id)
        .into_iter()
        .fold(own, |lowest, child| {
            lowest.max(lowest_edge(document, child))
        })
}
