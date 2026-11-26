use super::*;

#[test]
fn a_frame_says_which_host_state_it_was_laid_out_against() {
    let laid = frame(41);
    round_trips(&laid);
    let presented = PresentCommit {
        serial: laid.serial,
        repaint_after_micros: Some(16_000),
        animating: true,
        input_waiting: false,
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
            painted: vec![ScreenPlacement {
                screen: ScreenId(2),
                instance: EditorInstanceId(1),
                view: ViewId::MAIN,
                surface: 4,
                width: 30,
                height: 40,
                scale_factor_millis: 2000,
                raster_millis: 2000,
            }],
        }),
    };
    round_trips(&presented);
}
