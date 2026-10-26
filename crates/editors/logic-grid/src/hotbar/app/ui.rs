use block_editor_beui::be_block::BlockContent;
use logic_grid_types::CompiledLogicContent;

use beui::{Align, Direction, ItemSize};
use block_editor_beui::be_block::Item;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::icons::{ICON_FOLDER, ICON_TRASH};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{BlockLink, ChildTarget, ContentProjection, Editor};
use logic_grid_types::hotbar::{HotbarContent, HotbarSlot, SlotKind};
use uuid::Uuid;

const PADDING: Space = Space::Xl;
const INDENT: f32 = 16.0;
const ROW_SPACING: Space = Space::Sm;

type Pinned = ContentProjection<HotbarContent>;

#[derive(Clone, PartialEq)]
struct Row {
    depth: usize,
    kind: RowKind,
}

#[derive(Clone, PartialEq)]
enum RowKind {
    Note(String),
    Folder(String),
    Component {
        name: String,
        compiled: Uuid,
        block: Option<ChildTarget>,
    },
}

#[component]
pub fn HotbarView(editor: Editor) -> NodeId {
    let hotbar = editor.block_content::<HotbarContent>();
    let pinned = hotbar.project(|hotbar| hotbar.root().slots.to_vec());

    let rows = create_memo(move || {
        let mut rows = Vec::new();
        pinned.with(|slots| flatten(slots, 0, &mut rows));
        rows
    });
    let keys = create_memo(move || (0..rows.with(Vec::len)).collect::<Vec<usize>>());
    let empty = create_memo(move || rows.with(Vec::is_empty));
    let read_only = editor.read_only();

    view! {
        <Frame padding=PADDING>
            <List spacing=ROW_SPACING>
                <Show condition={empty}>
                    <Text style=TextStyle::CAPTION>
                        "Nothing is pinned yet. Compiling a grid pins the component it builds."
                    </Text>
                </Show>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=ROW_SPACING>
                        <ForEach keys={keys}>|index: usize| {
                            let row = create_memo(move || {
                                rows.with(|rows| rows.get(index).cloned())
                            });
                            view! {
                                <SlotRow
                                    index={index}
                                    editor={editor}
                                    hotbar={hotbar}
                                    row={row}
                                    read_only={read_only}
                                />
                            }
                        }</ForEach>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn SlotRow(
    index: usize,
    editor: Editor,
    hotbar: Pinned,
    row: Memo<Option<Row>>,
    read_only: Memo<bool>,
) -> NodeId {
    let indent = create_memo(move || {
        Space::Custom(row.with(|row| row.as_ref().map_or(0.0, |row| row.depth as f32 * INDENT)))
    });
    let note = create_memo(move || {
        row.with(|row| match row {
            Some(Row {
                kind: RowKind::Note(text),
                ..
            }) => text.clone(),
            _ => String::new(),
        })
    });
    let folder = create_memo(move || {
        row.with(|row| match row {
            Some(Row {
                kind: RowKind::Folder(name),
                ..
            }) => name.clone(),
            _ => String::new(),
        })
    });
    let is_folder = create_memo(move || !folder.get().is_empty());
    let component = create_memo(move || {
        row.with(|row| match row {
            Some(Row {
                kind: RowKind::Component { block, .. },
                ..
            }) => *block,
            _ => None,
        })
    });
    let is_component = create_memo(move || {
        row.with(|row| {
            matches!(
                row,
                Some(Row {
                    kind: RowKind::Component { .. },
                    ..
                })
            )
        })
    });
    let name = create_memo(move || {
        row.with(|row| match row {
            Some(Row {
                kind: RowKind::Component { name, .. },
                ..
            }) => name.clone(),
            _ => String::new(),
        })
    });
    let unpin = move || {
        let Some(compiled) = row.with(|row| match row {
            Some(Row {
                kind: RowKind::Component { compiled, .. },
                ..
            }) => Some(*compiled),
            _ => None,
        }) else {
            return;
        };
        if let Ok(edit) = hotbar.read(|hotbar| hotbar.root().unpin(compiled)) {
            hotbar.operate(edit);
        }
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Spacer width={indent} />
            <Show condition={is_folder}>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_FOLDER />
                </Text>
            </Show>
            <Show condition={is_folder}>
                <Text>{folder}</Text>
            </Show>
            <Show condition={is_component}>
                <BlockLink
                    editor={editor}
                    block={component}
                    fallback={name}
                    @test_id={format!("hotbar.slot.{index}")}
                />
            </Show>
            <Text style=TextStyle::CAPTION>{note}</Text>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Show condition={is_component}>
                <Button
                    icon_only=true
                    glyph=ICON_TRASH
                    label="Unpin"
                    disabled={read_only}
                    @test_id={format!("hotbar.slot.{index}.unpin")}
                    on_click={unpin}
                />
            </Show>
        </List>
    }
}

fn flatten(slots: &[Item<HotbarSlot>], depth: usize, rows: &mut Vec<Row>) {
    for slot in slots {
        let kind = match &slot.kind {
            SlotKind::Builtin { tool } => RowKind::Note(tool.clone()),
            SlotKind::Locked { name } => RowKind::Note(format!("{name} (locked)")),
            SlotKind::Folder { name } => RowKind::Folder(name.clone()),
            SlotKind::Component { name, compiled } => RowKind::Component {
                name: name.clone(),
                compiled: *compiled,
                block: Some(ChildTarget::new(
                    *compiled,
                    CompiledLogicContent::CONTENT_TYPE,
                )),
            },
        };
        rows.push(Row { depth, kind });
        flatten(&slot.slots, depth + 1, rows);
    }
}
