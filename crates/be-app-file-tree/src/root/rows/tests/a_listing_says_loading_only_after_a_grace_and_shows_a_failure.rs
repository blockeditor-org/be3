use super::*;

#[test]
fn a_listing_says_loading_only_after_a_grace_and_shows_a_failure() {
    let host = EditorHost::default();
    let query = BlockQuery::References(CONTAINER);
    let listing = Listing::watch(&Blocks::of(&host), query);
    let mut clock = Clock {
        now: listing.since + GRACE / 2,
        due: None,
    };
    assert_eq!(clock.listed(&listing), Err(None));
    assert_eq!(clock.due, Some(listing.since + GRACE));

    clock.now = listing.since + GRACE;
    assert_eq!(clock.listed(&listing), Err(Some("Loading…".to_owned())));

    host.fail_blocks(query, "Offline".to_owned());
    assert_eq!(clock.listed(&listing), Err(Some("Offline".to_owned())));
}
