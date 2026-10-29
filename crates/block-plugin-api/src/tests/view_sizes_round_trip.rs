use super::*;

#[test]
fn view_sizes_round_trip() {
    let message = FrameCommit {
        sizes: vec![
            ViewSize {
                screen: ScreenId(1),
                logical: Size {
                    width: 400.0,
                    height: 24.5,
                },
            },
            ViewSize {
                screen: ScreenId(2),
                logical: Size {
                    width: 200.0,
                    height: 300.0,
                },
            },
        ],
        ..frame(4)
    };
    round_trips(&message);
}
