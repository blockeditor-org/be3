use be_model::{Document, Model};

use be_block::{BlockContent, Root, SCHEMA, derived_metadata, schema_fingerprint};

use crate::{TextBlock, TextContent};

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "text")]
struct GrownText {
    language: crate::TextLanguage,
    indentation: crate::TextIndentation,
    body: be_model::Text,
    wrap: bool,
}

impl Root for GrownText {
    const CONTENT_TYPE: uuid::Uuid = TextContent::CONTENT_TYPE;
}

#[test]
fn content_is_stamped_with_the_shape_it_was_saved_in() {
    let text = TextBlock::of("hello");
    let stamped = be_block::BlockMetadata {
        derived: derived_metadata(&text),
        ..Default::default()
    };
    assert_eq!(
        stamped.get(&SCHEMA),
        Some(schema_fingerprint::<TextContent>())
    );

    assert_eq!(
        schema_fingerprint::<TextContent>(),
        schema_fingerprint::<TextContent>()
    );
    assert_ne!(
        schema_fingerprint::<TextContent>(),
        schema_fingerprint::<Document<GrownText>>(),
        "a new field did not change the shape"
    );
}
