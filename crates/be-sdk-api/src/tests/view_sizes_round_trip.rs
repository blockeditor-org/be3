use super::*;

#[test]
fn view_sizes_round_trip() {
    let message = LayoutCommit {
        sizes: vec![
            ViewSize {
                screen: ScreenId(1),
                logical: Vec2 { x: 400.0, y: 24.5 },
            },
            ViewSize {
                screen: ScreenId(2),
                logical: Vec2 { x: 200.0, y: 300.0 },
            },
        ],
        ..frame(4)
    };
    round_trips(&message);
}
