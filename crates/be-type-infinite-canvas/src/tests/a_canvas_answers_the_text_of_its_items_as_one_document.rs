use std::collections::HashSet;

use be_block::{PartReader, SearchDocument};

use super::*;

#[test]
fn a_canvas_answers_the_text_of_its_items_as_one_document() {
    let mut content = CanvasContent::default();
    for text in ["Launch plan", "", "  Budget  "] {
        let mut entity = CanvasEntity::new(
            CanvasShape::Text,
            CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
        );
        entity.text = text.into();
        content = canvas_run(
            &content,
            InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
                pixels: None,
            },
        );
    }
    content = canvas_run(
        &content,
        InfiniteCanvasOperation::Add {
            entity: Box::new(rectangle()),
            pixels: None,
        },
    );
    let parts = PartReader::new(|_, _, _| panic!("a canvas is loaded whole and reads no part"));
    assert_eq!(
        pollster::block_on(content.search_text(None, &HashSet::new(), parts)),
        Ok(vec![SearchDocument::whole(
            "Launch plan\nBudget".to_owned()
        )])
    );
}
