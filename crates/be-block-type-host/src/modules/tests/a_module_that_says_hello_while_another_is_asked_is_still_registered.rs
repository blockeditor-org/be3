use super::*;

#[test]
fn a_module_that_says_hello_while_another_is_asked_is_still_registered() {
    // Every test of the process names one build, as the server a test starts needs.
    be_lib_build_id::install("test");
    let sources = vec![
        in_process::<Slow>("slow").declaring(vec![FIRST]),
        in_process::<Late>("late").declaring(vec![SECOND]),
    ];
    let modules =
        futures_util::FutureExt::now_or_never(started(sources, be_block_client::Clock::real()))
            .expect(
                "started waited for a wake after the late module's Hello was already collected",
            );
    assert_eq!(modules.module_of(FIRST), Some(0));
    assert_eq!(modules.module_of(SECOND), Some(1));
}
