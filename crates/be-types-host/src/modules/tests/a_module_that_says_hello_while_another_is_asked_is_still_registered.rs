use super::*;

#[test]
fn a_module_that_says_hello_while_another_is_asked_is_still_registered() {
    let sources = vec![
        in_process::<Slow>("slow").declaring(vec![FIRST]),
        in_process::<Late>("late").declaring(vec![SECOND]),
    ];
    let modules = futures_util::FutureExt::now_or_never(started(sources));
    assert!(
        modules.is_some(),
        "started waited for a wake after the late module's Hello was already collected"
    );
    assert_eq!(kind_of(FIRST).map(|kind| kind.module), Some(0));
    assert_eq!(kind_of(SECOND).map(|kind| kind.module), Some(1));
}
