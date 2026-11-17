use super::*;

#[test]
fn a_sleep_until_a_moment_a_held_clock_reached_ends_on_the_next_pass() {
    be_lib_build_id::install("test");
    let mut modules = Modules::new(
        vec![in_process::<Sleeper>("sleeper").declaring(vec![FIRST])],
        be_block_client::Clock::real(),
    );
    let first = Instant::now();
    modules.hold_clock(Some(first));
    modules.launch_all();
    futures_util::FutureExt::now_or_never(modules.ready()).expect("the module said hello");
    modules.tick_at(0, first);
    // The clock is held a second on, which the module has not been told.
    modules.hold_clock(Some(first + Duration::from_secs(1)));
    let now = modules.now();
    // As the host's loop does: it wakes for the sleep's moment (`wake_at`) and ends what is due.
    for _ in 0..10 {
        modules.take();
        modules.tick_due();
    }
    let held = &modules.modules[0];
    assert_eq!(
        held.sleeping.as_ref().map(|(at, _)| *at),
        Some(u64::MAX),
        "a sleep until a moment the held clock had reached was never answered"
    );
    assert_eq!(
        held.ticked,
        Some(now),
        "the module was told the clock as its sleep ended"
    );
}
