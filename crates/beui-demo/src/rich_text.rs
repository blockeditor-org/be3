use std::sync::Arc;

use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::TextAreaState;
use beui::{ItemSize, NodeId};
use beui_macros::{component, sample};
use text_editor_core::{Attribute, TextBuffer};

use crate::ROW_SPACING;
use crate::collaboration::ConcurrentEditing;
use crate::sample::{Sample, ScrollPage};

const TEXT: &str = "Rich text\nSelect words and press a button above, or Ctrl+B, Ctrl+I and \
Ctrl+U. The text holds no markup: each style is a mark over its letters, so typing at the end \
of bold text stays bold.\nHeadings\nA heading covers its whole line. Press Enter at the end of \
one to start a body line.\n";
const EDITOR_HEIGHT: f32 = 260.0;

#[component]
pub(crate) fn RichTextPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Rich text editor" code={vec![RichTextEditor::SOURCE]}>
                <RichTextEditor />
            </Sample>
            <Sample title="Live editing" code={vec![ConcurrentEditing::SOURCE]}>
                <ConcurrentEditing rich=true />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn RichTextEditor() -> NodeId {
    let styled = [
        ("Rich text", Attribute::Heading, 1),
        ("Headings", Attribute::Heading, 2),
        ("no markup", Attribute::Italic, 1),
        ("Ctrl+B", Attribute::Code, 1),
        ("Ctrl+I", Attribute::Code, 1),
        ("Ctrl+U", Attribute::Code, 1),
        ("bold text stays bold", Attribute::Bold, 1),
        ("whole line", Attribute::Underline, 1),
    ];
    let formats: Vec<_> = styled
        .into_iter()
        .filter_map(|(words, attribute, value)| {
            let start = TEXT.find(words)?;
            Some((start..start + words.len(), attribute, value))
        })
        .collect();
    let document = Arc::new(TextBuffer::rich(TEXT, &formats));
    let state = TextAreaState::new(document as Arc<dyn text_editor_core::Document>);
    view! {
        <List spacing=ROW_SPACING>
            <FormatBar state={state} test_prefix="demo.rich" />
            <TextArea
                @test_id="demo.rich.editor"
                @sizing=ItemSize::Fixed(EDITOR_HEIGHT)
                state={state}
            />
        </List>
    }
}
