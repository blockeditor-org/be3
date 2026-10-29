use be_block;
use be_block::be_model::{Document as Content, Model, ObjectId, RichText};
use block_editor_plugin::Waker;
use text_editor_core::schema::basic::{BOLD, ITALIC};
use text_editor_core::{Document, Expand, Name, Scope, Value};
use uuid::Uuid;

use super::{FieldOf, LiveText};

mod a_format_goes_out_as_a_mark_and_a_peers_mark_lands_on_its_text;
mod peers_formatting_and_typing_at_once_converge;
mod plain_text_takes_no_formats;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Note {
    words: RichText,
    plain: be_block::be_model::Text,
}

impl be_block::Root for Note {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x5ee0_0000_0000_0000_0000_0000_0000_0001);
}

fn note(text: &str) -> Content<Note> {
    Content::new(&Note {
        words: RichText::from(text),
        plain: be_block::be_model::Text::from(text),
    })
}

fn live(content: &Content<Note>) -> LiveText<FieldOf<Note>> {
    let text = LiveText::new(FieldOf::rich(ObjectId::ROOT, Note::WORDS), Waker::default());
    text.adopt(content);
    text
}

fn change(key: &str, value: Option<Value>) -> text_editor_core::Change {
    text_editor_core::Change {
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
    let operations = text.take_operations();
    for operation in &operations {
        content.apply(operation);
    }
    operations
}
