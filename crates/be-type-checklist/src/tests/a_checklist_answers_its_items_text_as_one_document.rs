use std::collections::HashSet;

use be_block::{BlockContent, PartReader, SearchDocument};

use super::*;

#[test]
fn a_checklist_answers_its_items_text_as_one_document() {
    let mut content = ChecklistContent::default();
    for text in ["Oat milk", "Rye bread", "Call the plumber"] {
        content.apply(&Checklist::add(text).1);
    }
    let parts = PartReader::new(|_, _, _| panic!("a checklist is loaded whole and reads no part"));
    assert_eq!(
        pollster::block_on(content.search_text(None, &HashSet::new(), parts)),
        Ok(vec![SearchDocument::whole(
            "Oat milk\nRye bread\nCall the plumber".to_owned()
        )])
    );
}
