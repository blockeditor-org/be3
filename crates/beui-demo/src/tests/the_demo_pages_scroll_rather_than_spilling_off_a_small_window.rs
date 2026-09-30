use super::*;

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
            let mut test = demo(window);
            open(&mut test, page);
            for _ in 0..WHEEL_TICKS {
                test.scroll_at(over_the_page(window), Vec2::new(0.0, -WHEEL_STEP));
            }
            let root = test.document().root().expect("the demo built a root");
            let bottom = lowest_edge(test.document(), root, f32::INFINITY);
            assert!(
                bottom <= window.y,
                "{} reaches {bottom} in a {window:?} window",
                page.title()
            );
        }
    }
}
