use std::ops::Range;
use std::sync::Arc;

use beui::icons::{
    ICON_CHECKLIST, ICON_CODE, ICON_CODE_BLOCKS, ICON_FORMAT_BOLD, ICON_FORMAT_H1, ICON_FORMAT_H2,
    ICON_FORMAT_ITALIC, ICON_FORMAT_LIST_BULLETED, ICON_FORMAT_LIST_NUMBERED, ICON_FORMAT_QUOTE,
    ICON_FORMAT_STRIKETHROUGH, ICON_FORMAT_UNDERLINED,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{TextAreaState, VoidView};
use beui::{Direction, ItemSize, NodeId, TextAlign, Vec2};
use beui_macros::{component, sample};
use text_editor_core::schema::basic::{
    self, Basic, CHECK, CHECKED, CODE_BLOCK, LIST, MENTION, NUMBER, QUOTE,
};
use text_editor_core::{EditorCommand, MarkSet, Name, Seed, TextBuffer, Transform, VOID, Value};

use crate::ROW_SPACING;
use crate::collaboration::ConcurrentEditing;
use crate::sample::{Sample, ScrollPage};

const TEXT: &str = "Rich text\nThe document is plain text with marks beside it. Select words and \
press a button above, or Ctrl+B, Ctrl+I, Ctrl+U and Ctrl+E.\nBlocks\nA heading, a list item \
or a quote is a prop of its line, and the text area draws each line from it.\nPress \
Enter to continue a list\nTab nests an item\nEnter on an empty item leaves the list\nWrite a \
schema\nRender its blocks\nShip it\nA quote is drawn with a bar beside it.\nfn main() {\n    \
println!(\"decorated by the rust highlighter\");\n}\nAsk \u{fffc} about voids: the caret steps \
over one in a single move.\n";
const EDITOR_HEIGHT: f32 = 420.0;
const MENTION_CHAR: f32 = 7.5;
const MENTION_PADDING: f32 = 6.0;
const MENTION_HEIGHT: f32 = 20.0;

pub(crate) const GLYPHS: [(&str, &str); 12] = [
    (basic::BOLD, ICON_FORMAT_BOLD),
    (basic::ITALIC, ICON_FORMAT_ITALIC),
    (basic::UNDERLINE, ICON_FORMAT_UNDERLINED),
    (basic::STRIKETHROUGH, ICON_FORMAT_STRIKETHROUGH),
    (basic::CODE, ICON_CODE),
    ("heading1", ICON_FORMAT_H1),
    ("heading2", ICON_FORMAT_H2),
    (basic::BULLET, ICON_FORMAT_LIST_BULLETED),
    (NUMBER, ICON_FORMAT_LIST_NUMBERED),
    (CHECK, ICON_CHECKLIST),
    (QUOTE, ICON_FORMAT_QUOTE),
    (CODE_BLOCK, ICON_CODE_BLOCKS),
];

#[component]
pub(crate) fn RichTextPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Rich text editor"
                code={vec![
                    RichTextEditor::SOURCE,
                    Mention::SOURCE,
                    clear_formatting::SOURCE,
                ]}
            >
                <RichTextEditor />
            </Sample>
            <Sample title="Live editing" code={vec![ConcurrentEditing::SOURCE]}>
                <ConcurrentEditing rich=true />
            </Sample>
        </ScrollPage>
    }
}

fn line_of(text: &str, words: &str) -> Range<usize> {
    let start = text.find(words).unwrap_or(0);
    start..start + words.len()
}

pub(crate) fn editor_seeds() -> Vec<Seed> {
    let line = |words: &str| line_of(TEXT, words);
    let mention = TEXT.find(VOID).unwrap_or(0);
    vec![
        basic::block(line("Rich text"), basic::HEADING, Value::Int(1)),
        basic::block(line("Blocks"), basic::HEADING, Value::Int(2)),
        basic::mark(line("plain text"), basic::ITALIC),
        basic::mark(line("marks beside it"), basic::BOLD),
        basic::mark(line("Ctrl+B"), basic::CODE),
        basic::mark(line("Ctrl+I"), basic::CODE),
        basic::mark(line("Ctrl+U"), basic::CODE),
        basic::mark(line("Ctrl+E"), basic::CODE),
        basic::block(
            line("Press Enter to continue").start..line("leaves the list").end,
            LIST,
            Value::text(NUMBER),
        ),
        basic::block(line("Tab nests"), basic::INDENT, Value::Int(1)),
        basic::block(
            line("Write a schema").start..line("Ship it").end,
            LIST,
            Value::text(CHECK),
        ),
        basic::block(line("Write a schema"), CHECKED, Value::Bool(true)),
        basic::block(line("A quote is drawn"), QUOTE, Value::Bool(true)),
        basic::block(
            line("fn main").start..line("\n}\n").start + 2,
            CODE_BLOCK,
            Value::text("rust"),
        ),
        basic::mark_value(mention..mention + VOID.len(), MENTION, Value::text("ada")),
    ]
}

pub(crate) fn render_void() -> RenderFn<VoidView> {
    RenderFn::new(|void: VoidView| {
        view! {
            <Mention void />
        }
    })
}

pub(crate) fn mention_size(props: &MarkSet) -> Vec2 {
    let name = props
        .get(MENTION)
        .and_then(Value::as_text)
        .unwrap_or_default();
    Vec2::new(
        (name.chars().count() + 1) as f32 * MENTION_CHAR + MENTION_PADDING * 2.0,
        MENTION_HEIGHT,
    )
}

#[sample]
#[component]
fn RichTextEditor() -> NodeId {
    let document = Arc::new(TextBuffer::rich(TEXT, &editor_seeds()));
    let state = TextAreaState::with_schema(document, Arc::new(Basic));
    let mention = MarkSet::new([(Name::new(MENTION), Value::text("grace"))]);
    view! {
        <List spacing=ROW_SPACING>
            <List direction=Direction::Horizontal spacing=ROW_SPACING wrap=true>
                <FormatBar state glyphs={GLYPHS.to_vec()} test_prefix="demo.rich" />
                <Button
                    @test_id="demo.rich.mention"
                    label="Mention"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        state.execute(EditorCommand::InsertVoid(mention.clone()));
                        state.focus();
                    }}
                />
                <Button
                    @test_id="demo.rich.clear"
                    label="Clear formatting"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        state.execute(EditorCommand::Transform(clear_formatting));
                        state.focus();
                    }}
                />
            </List>
            <TextArea
                @test_id="demo.rich.editor"
                @sizing=ItemSize::Fixed(EDITOR_HEIGHT)
                state
                render_void={render_void()}
                void_size={Some(mention_size as fn(&MarkSet) -> Vec2)}
            />
        </List>
    }
}

#[sample]
fn clear_formatting(transform: &mut Transform<'_>) {
    let Some(formats) = transform.formats() else {
        return;
    };
    for range in transform.selections().to_vec() {
        let mut keys: Vec<Name> = formats
            .marks_in(range.clone())
            .flat_map(|(_, marks)| marks.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>())
            .filter(|key| key.as_str() != MENTION)
            .collect();
        keys.dedup();
        for key in keys {
            transform.set_mark(range.clone(), &key, None);
        }
    }
}

#[sample]
#[component]
fn Mention(void: VoidView) -> NodeId {
    let name = create_memo(move || {
        void.props.with(|props| {
            format!(
                "@{}",
                props
                    .get(MENTION)
                    .and_then(Value::as_text)
                    .unwrap_or_default()
            )
        })
    });
    view! {
        <Frame
            @test_id={format!("demo.rich.{}", name.get_untracked())}
            fill=ColorRole::AccentSoft
            radius=Radius::Md
        >
            <Text align=TextAlign::Center vertical_align=TextAlign::Center>{name}</Text>
        </Frame>
    }
}
