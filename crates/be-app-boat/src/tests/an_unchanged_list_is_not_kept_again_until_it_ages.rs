use super::*;

#[test]
fn an_unchanged_list_is_not_kept_again_until_it_ages() {
    let mut boat = Boat::listing(SANDBOXES);
    let first = boat.kept().expect("the first list is kept").fetched;

    boat.wait(Duration::from_secs(15));
    let poll = boat.fetch();
    boat.answer(&poll, 200, SANDBOXES);
    assert_eq!(boat.kept().map(|kept| kept.fetched), Some(first));

    for _ in 0..20 {
        boat.wait(Duration::from_secs(15));
        let poll = boat.fetch();
        boat.answer(&poll, 200, SANDBOXES);
    }
    let later = boat.kept().expect("still kept").fetched;
    assert!(later >= first + 5 * 60, "kept again once five minutes old");

    boat.wait(Duration::from_secs(15));
    let changed = boat.fetch();
    boat.answer(&changed, 200, EMPTY);
    assert_eq!(boat.kept().map(|kept| kept.sandboxes.len()), Some(0));
}
