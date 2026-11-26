use super::*;

/// One who may read the block (its hosts) sees the whole world: the gems still buried as well
/// as the cells dug, and may hide the gems again.
#[test]
fn the_world_shows_one_who_may_read_it_where_every_gem_lies() {
    let mut test = BeuiTest::<HuntRoot>::editing(Uuid::new_v4());
    test.store().register(be_type_gem_hunt::registrations());
    let me = test.host().account_id();
    test.hold(None, world(me));
    test.run();
    assert_eq!(shown(&test, "hunt.summary"), "\"1 of 6 gems found.\"");
    test.snapshot("the_world_shows_one_who_may_read_it_where_every_gem_lies");

    test.click("hunt.new-world");
    let hunt = test.content::<HuntContent>(None).root();
    assert!(hunt.dug.is_empty(), "a new world has nothing dug");
    assert_eq!(hunt.gems.len(), be_type_gem_hunt::GEMS as usize);
    assert_eq!(shown(&test, "hunt.summary"), "\"0 of 6 gems found.\"");
}
