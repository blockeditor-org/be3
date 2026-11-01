use super::*;

const MILLISECOND: Duration = Duration::from_millis(1);

#[test]
fn a_frame_waits_for_plugins_until_the_next_present_less_its_own_composite() {
    let now = Instant::now();
    let refresh = Duration::from_micros(16_667);
    let mut host = Host::new();
    host.present = Some((now + refresh, refresh));
    host.composites = VecDeque::from([3 * MILLISECOND, 5 * MILLISECOND, 2 * MILLISECOND]);
    let left = host
        .frame_deadline()
        .remaining()
        .expect("the deadline is ahead");
    assert!(
        left <= refresh - 6 * MILLISECOND && left > refresh - 8 * MILLISECOND,
        "the host keeps its longest recent composite and a millisecond to submit before the present, {left:?}"
    );

    host.present = Some((now + MILLISECOND, refresh));
    let left = host
        .frame_deadline()
        .remaining()
        .expect("the deadline is ahead");
    assert!(
        left > 8 * MILLISECOND && left <= refresh + MILLISECOND - 6 * MILLISECOND,
        "a frame too late for this present waits for the one after it rather than not at all, {left:?}"
    );
}
