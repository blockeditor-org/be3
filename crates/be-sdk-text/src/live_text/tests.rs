use be_block;
use be_block::be_block_model::{Document as Content, Model, ObjectId, RichText};
use be_sdk::Waker;
use beui_text_editor_core::schema::basic::{BOLD, ITALIC};
use beui_text_editor_core::{Document, Expand, Name, Scope, Value};
use uuid::Uuid;

use super::{FieldOf, LiveText, Outgoing};

mod a_format_goes_out_as_a_mark_and_a_peers_mark_lands_on_its_text;
mod peers_formatting_and_typing_at_once_converge;
mod plain_text_takes_no_formats;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Note {
    words: RichText,
    plain: be_block::be_block_model::Text,
}

impl be_block::Root for Note {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x5ee0_0000_0000_0000_0000_0000_0000_0001);
}

fn note(text: &str) -> Content<Note> {
    Content::new(&Note {
        words: RichText::from(text),
        plain: be_block::be_block_model::Text::from(text),
    })
}

fn live(content: &Content<Note>) -> LiveText<FieldOf<Note>> {
    let text = LiveText::new(FieldOf::rich(ObjectId::ROOT, Note::WORDS), Waker::default());
    text.adopt(content);
    text
}

fn change(key: &str, value: Option<Value>) -> beui_text_editor_core::Change {
    beui_text_editor_core::Change {
        scope: Scope::Inline,
        expand: Expand::After,
        key: Name::new(key),
        value,
    }
}

fn formatted(text: &LiveText<FieldOf<Note>>, index: usize, key: &str) -> bool {
    text.read()
        .and_then(|read| read.formats())
        .and_then(|formats| formats.marks_at(index).cloned())
        .is_some_and(|marks| marks.has(key, &Value::Bool(true)))
}

fn send(text: &LiveText<FieldOf<Note>>, content: &mut Content<Note>) -> Vec<be_block::Edit> {
    let operations = text
        .take_outgoing()
        .into_iter()
        .filter_map(|outgoing| match outgoing {
            Outgoing::Edit(edit) => Some(edit),
            Outgoing::Break => None,
        })
        .collect::<Vec<_>>();
    for operation in &operations {
        content.apply(operation);
    }
    operations
}
