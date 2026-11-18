use super::*;

#[test]
fn a_frame_waits_for_its_child_and_shows_it() {
    let mut root = root();
    for time in [1, 2] {
        let done = root
            .frame(
                FrameRequest {
                    time,
                    wall_clock: 0,
                },
                Instant::now() + PATIENT,
            )
            .expect("the root draws");
        assert_eq!(
            shown(&done),
            vec![ROOT_LAYER, CHILD_LAYER],
            "frame {time} waited for its child's answer, so it shows the child over the root"
        );
    }
}
