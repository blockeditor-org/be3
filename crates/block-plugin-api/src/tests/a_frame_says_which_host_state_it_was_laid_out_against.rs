use super::*;

#[test]
fn a_frame_says_which_host_state_it_was_laid_out_against() {
    let configure = Message::Configure { serial: 41 };
    assert_eq!(configure.direction(), Direction::ToPlugin);
    assert_eq!(
        decode_frame(&encode_frame(&configure).unwrap()).unwrap(),
        configure
    );

    let frame = Message::Frame(FrameCommit {
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
    });
    assert_eq!(frame.direction(), Direction::ToHost);
    assert_eq!(decode_frame(&encode_frame(&frame).unwrap()).unwrap(), frame);
}
