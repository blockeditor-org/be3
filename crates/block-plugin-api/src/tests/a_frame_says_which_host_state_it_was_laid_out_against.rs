use super::*;

#[test]
fn a_frame_says_which_host_state_it_was_laid_out_against() {
    let frame = FrameCommit {
        repaint_after_micros: Some(16_000),
        animating: true,
        presented: Some(PresentedFrame {
            sequence: 3,
            damage: vec![ScreenDamage {
                screen: ScreenId(2),
                rect: SurfaceRect {
                    x: 1,
                    y: 2,
                    width: 30,
                    height: 40,
                },
            }],
        }),
        ..frame(41)
    };
    round_trips(&frame);
}
