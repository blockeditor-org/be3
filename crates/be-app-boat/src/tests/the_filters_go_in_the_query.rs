use super::*;

use be_sdk_beui::beui::{Key, Modifiers};

#[test]
fn the_filters_go_in_the_query() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.test.click("boat.filter.label");
    boat.test.type_text("env=ci");
    boat.test.key(Key::Enter, Modifiers::NONE);
    boat.test.run();
    let labelled = boat.fetch();
    assert_eq!(labelled.request.url, format!("{LIST}&label=env%3Dci"));
    boat.answer(&labelled, 200, LAST_PAGE);

    boat.test.click("boat.filter.prefix");
    boat.test.type_text("nightly-");
    boat.test.key(Key::Enter, Modifiers::NONE);
    boat.test.run();
    let both = boat.fetch();
    assert_eq!(
        both.request.url,
        format!("{LIST}&namePrefix=nightly-&label=env%3Dci")
    );
    let boat_block = boat.test.content::<BoatContent>(None).root();
    assert_eq!(boat_block.name_prefix, "nightly-");
    assert_eq!(boat_block.label, "env=ci");
}
