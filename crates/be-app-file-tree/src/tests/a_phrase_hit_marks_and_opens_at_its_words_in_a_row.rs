use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery, Search, SearchHit, SearchResults};

use super::*;

#[test]
#[allow(clippy::single_range_in_vec_init)] // a list of ranges, which here holds one
fn a_phrase_hit_marks_and_opens_at_its_words_in_a_row() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let (trip, text) = (Uuid::from_u128(3), Uuid::from_u128(7));
    let mut found = BlockInfo::new(trip, text, BlockParent::Block(MY_FILES));
    found.set_intrinsic_name("Trip".to_owned());
    my_files(&mut fixture, vec![found.clone()]);
    fixture
        .host
        .set_blocks(BlockQuery::Block(trip), vec![found]);
    let typed = "\"big river\"";
    let snippet = "The river was low. Further on, the Big River met the delta.";
    let start = snippet.find("Big").unwrap();
    fixture.test.click("file-tree.filter");
    fixture.settle();
    fixture.test.type_text(typed);
    fixture.settle();
    // The index marks a phrase where its words are in a row, as one match, never a word of it
    // alone (`Query::matches`).
    fixture.host.set_search(
        Search {
            text: typed.to_owned(),
            within: None,
        },
        SearchResults {
            hits: vec![SearchHit {
                block: trip,
                key: String::new(),
                matches: vec![start..start + "Big River".len()],
                snippet: snippet.to_owned(),
            }],
            complete: true,
        },
    );
    fixture.settle();

    assert!(fixture.test.shown(&format!("file-tree.hit.{trip}")));
    fixture
        .test
        .snapshot("a_phrase_hit_marks_and_opens_at_its_words_in_a_row");

    fixture.test.click(format!("file-tree.hit.{trip}"));
    fixture.settle();
    let opened = fixture.test.take_opens();
    assert!(
        opened.iter().any(|open| open.block_id == trip
            && open.at
                == Some(be_sdk_beui::SearchLocation {
                    key: String::new(),
                    text: "Big River".to_owned(),
                })),
        "the hit opens at the phrase, not at an earlier word of it: {opened:?}"
    );
}
