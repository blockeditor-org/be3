use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn a_plugin_repaint_deadline_wakes_the_host_on_every_frame_until_it_is_due() {
    let mut pacing = Pacing::default();
    pacing.ready(ms(1000), Some(ms(500)));
    assert_eq!(pacing.wake_after(ms(1000), true), Some(ms(500)));
    assert_eq!(
        pacing.wake_after(ms(1016), true),
        Some(ms(484)),
        "a host frame after the one that received the deadline still asks for it",
    );
    assert_eq!(pacing.wake_after(ms(1016), false), None);
    assert!(!pacing.due(ms(1016), 2));

    assert!(pacing.due(ms(1500), 3));
    pacing.request(ms(1500), 3);
    assert_eq!(pacing.wake_after(ms(1500), true), Some(FRAME_TIMEOUT));
    assert!(!pacing.due(ms(1516), 4), "a frame is not asked for twice");

    pacing.ready(ms(1520), None);
    assert_eq!(pacing.wake_after(ms(1520), true), None);
    assert!(!pacing.due(ms(3000), 5));
}
