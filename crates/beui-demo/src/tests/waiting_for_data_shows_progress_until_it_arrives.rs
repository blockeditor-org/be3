use super::*;

#[test]
fn waiting_for_data_shows_progress_until_it_arrives() {
    let mut test = alone(WIDE, Page::Inputs);
    let root = test.document().root().expect("the demo built a root");
    reveal(&mut test, "demo.loading.arrive");
    assert!(
        !test.shown("demo.loading.content"),
        "nothing is shown before it arrives"
    );

    test.click("demo.loading.arrive");
    test.frame(Vec::new());
    assert!(test.shown("demo.loading.content"));
    assert_eq!(showing(test.document(), root, "Hello, the data is here"), 1);
}
