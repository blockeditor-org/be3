use super::*;

#[test]
fn connecting_checks_the_key_and_keeps_it_for_its_owner_alone() {
    let mut boat = Boat::new();
    boat.connect();

    let refused = boat.fetch();
    assert_eq!(refused.request.method, "GET");
    assert_eq!(refused.request.url, "https://boat.dev/api/v1/me");
    boat.answer(&refused, 401, UNAUTHORIZED);
    assert!(boat.test.shown("boat.sign-in.problem"));
    boat.test
        .snapshot("a_key_boat_refuses_says_so_on_the_sign_in_page");
    assert!(
        boat.settings_blocks().is_empty(),
        "a refused key is not kept"
    );

    boat.test.click("boat.sign-in.connect");
    boat.test.run();
    let me = boat.fetch();
    boat.answer(&me, 200, ME);

    let kept = boat.settings_blocks();
    assert_eq!(kept.len(), 1);
    assert!(boat.test.store().owner_only(kept[0]));
    let key = boat
        .test
        .content::<BoatSettingsContent>(Some(kept[0]))
        .root()
        .key("")
        .cloned()
        .expect("the key is kept for the block's own account");
    assert_eq!(key.secret, SECRET);
    assert_eq!(key.login, "Sam Sailor (sam@example.com)");
    let year = 365 * 24 * 60 * 60;
    let now = crate::time::now();
    assert!((key.expires - now - year).abs() < 24 * 60 * 60);

    let list = boat.fetch();
    assert_eq!(list.request.url, LIST);
    assert!(boat.test.shown("boat.list"));
}
