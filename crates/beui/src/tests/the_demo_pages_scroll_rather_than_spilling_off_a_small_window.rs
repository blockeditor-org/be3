#![allow(dead_code)]

use super::*;

include!("../../examples/demo.rs");

const WINDOWS: [Vec2; 4] = [
    Vec2::new(360.0, 700.0),
    Vec2::new(500.0, 500.0),
    Vec2::new(900.0, 900.0),
    Vec2::new(1200.0, 500.0),
];
const WHEEL_STEP: f32 = 200.0;
const WHEEL_TICKS: usize = 20;

#[test]
fn the_demo_pages_scroll_rather_than_spilling_off_a_small_window() {
    for window in WINDOWS {
        for page in PAGES {
            let mut harness = Harness::sized(DemoApp::new().document, window);
            harness.frame(Vec::new());
            open_demo_page(&mut harness, page.title());
            let over_the_page = pos2(window.x * 0.7, window.y * 0.6);
            for _ in 0..WHEEL_TICKS {
                harness.scroll(over_the_page, Vec2::new(0.0, -WHEEL_STEP), Modifiers::NONE);
            }
            harness.frame(Vec::new());
            let root = harness.document.root().expect("the demo built a root");
            let bottom = lowest_edge(harness.document(), root, f32::INFINITY);
            assert!(
                bottom <= window.y,
                "{} reaches {bottom} in a {window:?} window",
                page.title()
            );
        }
    }
}

fn lowest_edge(document: &Document, id: NodeId, clip: f32) -> f32 {
    let rect = document.node_rect(id);
    let own = rect.map_or(0.0, |rect| rect.bottom().min(clip));
    let scrolls = document.first_offset_within(id) == Some(id);
    let clip = match rect.filter(|_| scrolls) {
        Some(rect) => rect.bottom().min(clip),
        None => clip,
    };
    document
        .children(id)
        .into_iter()
        .fold(own, |lowest, child| {
            lowest.max(lowest_edge(document, child, clip))
        })
}
