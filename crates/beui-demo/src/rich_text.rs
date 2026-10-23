use std::ops::Range;
use std::sync::Arc;

use beui::icons::{
    ICON_BOLD, ICON_CODE, ICON_HEADING_1, ICON_HEADING_2, ICON_ITALIC, ICON_LIST, ICON_LIST_CHECKS,
    ICON_LIST_ORDERED, ICON_QUOTE, ICON_SQUARE_CODE, ICON_STRIKETHROUGH, ICON_UNDERLINE,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{BlockView, TextAreaState, VoidView};
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
or a quote is a prop of its line, and the page draws each line with its own render_block.\nPress \
Enter to continue a list\nTab nests an item\nEnter on an empty item leaves the list\nWrite a \
schema\nRender its blocks\nShip it\nA quote is drawn with a bar beside it.\nfn main() {\n    \
println!(\"decorated by the rust highlighter\");\n}\nAsk \u{fffc} about voids: the caret steps \
over one in a single move.\n";
const EDITOR_HEIGHT: f32 = 420.0;
const LEAD_WIDTH: f32 = 28.0;
const INDENT_WIDTH: f32 = 24.0;
const BULLET_SIZE: f32 = 6.0;
const QUOTE_BAR: f32 = 3.0;
const QUOTE_GAP: f32 = 10.0;
const CODE_PADDING: f32 = 8.0;
const MENTION_CHAR: f32 = 7.5;
const MENTION_PADDING: f32 = 6.0;
const MENTION_HEIGHT: f32 = 20.0;

pub(crate) const GLYPHS: [(&str, &str); 12] = [
    (basic::BOLD, ICON_BOLD),
    (basic::ITALIC, ICON_ITALIC),
    (basic::UNDERLINE, ICON_UNDERLINE),
    (basic::STRIKETHROUGH, ICON_STRIKETHROUGH),
    (basic::CODE, ICON_CODE),
    ("heading1", ICON_HEADING_1),
    ("heading2", ICON_HEADING_2),
    (basic::BULLET, ICON_LIST),
    (NUMBER, ICON_LIST_ORDERED),
    (CHECK, ICON_LIST_CHECKS),
    (QUOTE, ICON_QUOTE),
    (CODE_BLOCK, ICON_SQUARE_CODE),
];

#[component]
pub(crate) fn RichTextPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Rich text editor"
                code={vec![
                    RichTextEditor::SOURCE,
                    RichBlock::SOURCE,
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
        basic::mark(line("render_block"), basic::CODE),
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

pub(crate) fn render_block() -> RenderFn<BlockView> {
    RenderFn::new(|block: BlockView| {
        view! {
            <RichBlock block />
        }
    })
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
                render_block={render_block()}
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
fn RichBlock(block: BlockView) -> NodeId {
    let BlockView {
        state,
        line,
        start,
        props,
        line_height,
        text,
    } = block;
    let list = create_memo(move || {
        props.with(|props| props.get(LIST).and_then(Value::as_text).map(str::to_owned))
    });
    let listed = create_memo(move || list.get().is_some());
    let quoted = create_memo(move || props.with(|props| props.get(QUOTE).is_some()));
    let code = create_memo(move || props.with(|props| props.get(CODE_BLOCK).is_some()));
    let indent = create_memo(move || props.with(basic::depth) as f32 * INDENT_WIDTH);
    let lead = create_memo(move || match listed.get() {
        true => Some(LEAD_WIDTH + indent.get()),
        false => Some(0.0),
    });
    let marker = create_memo(move || Sides::new(indent.get(), 0.0, 0.0, 0.0));
    let first_line = create_memo(move || Some(line_height.get()));
    let fill = create_memo(move || match code.get() {
        true => ColorRole::Surface,
        false => ColorRole::None,
    });
    let padding = create_memo(move || match code.get() {
        true => Sides::horizontal(CODE_PADDING),
        false => Sides::ZERO,
    });
    view! {
        <Frame fill padding>
            <List direction=Direction::Horizontal spacing=0.0>
                <Show condition={quoted}>
                    <Frame padding={Sides::new(0.0, 0.0, QUOTE_GAP, 0.0)}>
                        <Frame width=QUOTE_BAR fill=ColorRole::Accent />
                    </Frame>
                </Show>
                <Frame width=lead padding=marker align={Align2::new(Align::Start, Align::Start)}>
                    <Frame height=first_line align={Align2::new(Align::Start, Align::Center)}>
                        <List spacing=0.0>
                            <Show condition={listed}>
                                <ListMarker state line start props kind={list} />
                            </Show>
                        </List>
                    </Frame>
                </Frame>
                <Frame @sizing=ItemSize::Percent(100.0)>{text}</Frame>
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn ListMarker(
    state: TextAreaState,
    line: ReadSignal<usize>,
    start: Memo<usize>,
    props: Memo<MarkSet>,
    kind: Memo<Option<String>>,
) -> NodeId {
    let bullet = create_memo(move || kind.get().as_deref() == Some(basic::BULLET));
    let number = create_memo(move || kind.get().as_deref() == Some(NUMBER));
    let check = create_memo(move || kind.get().as_deref() == Some(CHECK));
    let ordinal = create_memo(move || {
        state.content().get();
        let at = line.get();
        let ordinal = state.with_snapshot(|snapshot| {
            snapshot
                .formats
                .as_ref()
                .map_or(1, |formats| basic::ordinal(formats, at))
        });
        format!("{ordinal}.")
    });
    let checked = create_memo(move || props.with(|props| props.get(CHECKED).is_some()));
    let toggle = move |on: bool| {
        let at = state.with_core(|core| core.position(start.get_untracked()));
        state.execute(EditorCommand::SetBlock {
            at: Some(at),
            key: Name::new(CHECKED),
            value: on.then_some(Value::Bool(true)),
        });
    };
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Show condition={bullet}>
                <Frame
                    width=BULLET_SIZE
                    height=BULLET_SIZE
                    fill=ColorRole::Text
                    radius=Radius::Full
                />
            </Show>
            <Show condition={number}>
                <Text color=TextColor::Muted>{ordinal}</Text>
            </Show>
            <Show condition={check}>
                <Checkbox
                    @test_id={format!("demo.rich.checkbox.{}", line.get_untracked())}
                    checked={Binding::new(checked, toggle)}
                    capture_presses=true
                    tab_stop=false
                    press_focus=false
                />
            </Show>
        </List>
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
