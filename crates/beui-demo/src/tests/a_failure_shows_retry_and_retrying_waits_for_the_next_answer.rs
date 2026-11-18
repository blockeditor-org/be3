use super::*;

#[test]
fn a_failure_shows_retry_and_retrying_waits_for_the_next_answer() {
    let mut test = alone(WIDE, Page::Inputs);
    let root = test.document().root().expect("the demo built a root");
    reveal(&mut test, "demo.loading.fail");
    test.click("demo.loading.fail");
    test.frame(Vec::new());
    reveal(&mut test, "demo.loading.retry");
    assert_eq!(showing(test.document(), root, "The server said no"), 1);
    test.snapshot("a_failure_shows_retry_and_retrying_waits_for_the_next_answer");

    test.click("demo.loading.retry");
    test.frame(Vec::new());
    assert!(!test.shown("demo.loading.retry"), "retrying asks again");
    assert_eq!(showing(test.document(), root, "Hello, the data is here"), 0);

    test.click("demo.loading.arrive");
    test.frame(Vec::new());
    assert!(test.shown("demo.loading.content"));
    assert_eq!(showing(test.document(), root, "Hello, the data is here"), 1);
}
