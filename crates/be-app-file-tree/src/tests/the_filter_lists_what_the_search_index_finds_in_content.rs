use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery, Search, SearchHit, SearchResults};

use super::*;

#[test]
fn the_filter_lists_what_the_search_index_finds_in_content() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let (plan, notes) = (Uuid::from_u128(3), Uuid::from_u128(5));
    let text = Uuid::from_u128(7);
    let mut listed = BlockInfo::new(notes, text, BlockParent::Block(MY_FILES));
    listed.set_intrinsic_name("Notes".to_owned());
    my_files(&mut fixture, vec![listed.clone()]);
    fixture.settle();

    fixture.test.click("file-tree.filter");
    fixture.settle();
    fixture.test.type_text("launch");
    fixture.settle();
    assert!(fixture.says("Nothing matches"));
    assert!(
        !fixture.test.shown("file-tree.content-hits"),
        "content hits show before the index answers"
    );

    let mut planned = BlockInfo::new(plan, text, BlockParent::Block(MY_FILES));
    planned.set_intrinsic_name("Plan".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::Block(plan), vec![planned]);
    fixture
        .host
        .set_blocks(BlockQuery::Block(notes), vec![listed]);
    let snippet = "We launch on Friday, after the review.".to_owned();
    fixture.host.set_search(
        Search {
            text: "launch".to_owned(),
            within: None,
        },
        SearchResults {
            hits: vec![
                SearchHit {
                    block: plan,
                    key: String::new(),
                    matches: vec![3..9],
                    snippet,
                },
                SearchHit {
                    block: notes,
                    key: String::new(),
                    matches: vec![13..19],
                    snippet: "Rehearse the launch demo twice.".to_owned(),
                },
            ],
            complete: true,
        },
    );
    fixture.settle();
    assert!(fixture.test.shown("file-tree.content-hits"));
    for hit in [plan, notes] {
        assert!(fixture.test.shown(&format!("file-tree.hit.{hit}")));
    }
    assert!(fixture.says("We launch on Friday"));
    fixture.test.snapshot("the_filter_lists_hits_in_content");

    fixture.test.click(format!("file-tree.hit.{plan}"));
    fixture.settle();
    let opened = fixture.test.take_opens();
    assert!(
        opened.iter().any(|open| open.block_id == plan
            && open.at
                == Some(be_sdk_beui::SearchLocation {
                    key: String::new(),
                    text: "launch".to_owned(),
                })),
        "choosing a hit opens its block at the words it matched: {opened:?}"
    );
}
