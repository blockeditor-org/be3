use super::*;

#[test]
fn a_block_without_a_key_shows_how_to_make_one() {
    let mut boat = Boat::new();

    assert!(boat.test.shown("boat.sign-in"));
    assert_eq!(
        boat.label("boat.sign-in.actions"),
        "sandbox.read, sandbox.resume, sandbox.stop, account.read, exec, file.write"
    );
    assert!(!boat.test.has_fetches(), "nothing is asked without a key");

    boat.test.click("boat.sign-in.open");
    boat.test.run();
    assert_eq!(
        boat.test.take_opened_urls(),
        ["https://boat.dev/dashboard?tab=api-keys"]
    );
    boat.test
        .snapshot("a_block_without_a_key_shows_how_to_make_one");
}
