use super::*;
use crate::reactive::*;

#[test]
fn a_zero_delay_timer_started_during_a_frame_fires_in_that_frame() {
    let _root = crate::reactive::RootScope::new();
    let second_ran = create_signal(false);
    let document = build_node(move || {
        let second = create_timer(move || {
            second_ran.set(true);
            None
        });
        let first = create_timer(move || {
            second.start(Duration::ZERO);
            None
        });
        first.start(Duration::ZERO);
        view! {
            <List spacing=0.0>
                <Frame height=10.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);

    let frame = harness.frame(Vec::new());
    assert!(
        second_ran.get_untracked(),
        "work due now that the frame's own timers started is done in the frame, not left to a \
         next frame that a stopped clock never draws"
    );
    assert_eq!(
        frame.repaint_after,
        Duration::MAX,
        "nothing is left for another frame"
    );
}
