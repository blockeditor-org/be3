use super::*;

/// A module told the held moment that sleeps until it is woken on the host's next pass with
/// the clock yielded again in the same step: it runs its timers as a clock value arrives, so a
/// sleep ended without one would leave them waiting.
#[test]
fn a_due_sleep_ends_with_the_clock_yielded_again() {
    be_lib_build_id::install("test");
    let mut modules = Modules::new(
        vec![in_process::<Ticked>("ticked").declaring(vec![FIRST])],
        be_block_client::Clock::real(),
    );
    let held = Instant::now();
    modules.hold_clock(Some(held));
    modules.launch_all();
    futures_util::FutureExt::now_or_never(modules.ready()).expect("the module said hello");
    let now = modules.now();
    for _ in 0..10 {
        modules.take();
    }
    assert!(
        modules.modules[0].sleeping.is_some(),
        "the module sleeps until the moment it was told"
    );
    assert_eq!(modules.modules[0].ticked, Some(now));

    modules.tick_due();
    let said: Vec<String> = (0..10)
        .flat_map(|_| modules.take())
        .filter_map(|(_, heard)| match heard {
            Heard::Call {
                call: Call::Error { message, .. },
                ..
            } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(
        said,
        vec![format!("{:?}", vec![now])],
        "the sleep ended in the same step as the clock reached the module again"
    );
}
