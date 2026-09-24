use std::rc::Rc;

use block::Block;
use block_client::blocks::compiled_logic::CompiledLogic;
use block_editor_plugin::be_block::Item;
use block_editor_plugin::be_block::hotbar::{HotbarContent, HotbarSlot, SlotKind};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{ICON_DELETE, ICON_FOLDER};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{Body, Caption, Icon, IconButton, Scroll, use_theme};
use block_editor_plugin::{BlockLink, ChildTarget, ContentProjection, Editor};
use uuid::Uuid;

const PADDING: f32 = 16.0;
const INDENT: f32 = 16.0;
const ROW_SPACING: f32 = 6.0;

type Pinned = Rc<ContentProjection<HotbarContent>>;

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

    let rows = create_memo(clone!(pinned -> move || {
        let mut rows = Vec::new();
        pinned.with(|slots| flatten(slots, 0, &mut rows));
        rows
    }));
    let keys =
        create_memo(clone!(rows -> move || (0..rows.with(Vec::len)).collect::<Vec<usize>>()));
    let empty = create_memo(clone!(rows -> move || rows.with(Vec::is_empty)));
    let read_only = editor.read_only();

    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=ROW_SPACING>
                <Show condition={empty}>
                    <Caption
                        content="Nothing is pinned yet. Compiling a grid pins the component it builds."
                    />
                </Show>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=ROW_SPACING>
                        <ForEach keys={keys}>
                            {move |index: usize| {
                                let row = create_memo(clone!(rows -> move || {
                                    rows.with(|rows| rows.get(index).cloned())
                                }));
                                view! {
                                    <SlotRow
                                        index={index}
                                        editor={editor.clone()}
                                        hotbar={hotbar.clone()}
                                        row={row}
                                        read_only={read_only.clone()}
                                    />
                                }
                            }}
                        </ForEach>
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
    let indent = create_memo(clone!(row -> move || {
        ItemSize::Fixed(row.with(|row| row.as_ref().map_or(0.0, |row| row.depth as f32 * INDENT)))
    }));
    let note = create_memo(clone!(row -> move || row.with(|row| match row {
        Some(Row { kind: RowKind::Note(text), .. }) => text.clone(),
        _ => String::new(),
    })));
    let folder = create_memo(clone!(row -> move || row.with(|row| match row {
        Some(Row { kind: RowKind::Folder(name), .. }) => name.clone(),
        _ => String::new(),
    })));
    let is_folder = create_memo(clone!(folder -> move || !folder.get().is_empty()));
    let component = create_memo(clone!(row -> move || row.with(|row| match row {
        Some(Row { kind: RowKind::Component { block, .. }, .. }) => *block,
        _ => None,
    })));
    let is_component = create_memo(clone!(row -> move || {
        row.with(|row| matches!(row, Some(Row { kind: RowKind::Component { .. }, .. })))
    }));
    let name = create_memo(clone!(row -> move || row.with(|row| match row {
        Some(Row { kind: RowKind::Component { name, .. }, .. }) => name.clone(),
        _ => String::new(),
    })));
    let unpin = clone!(row hotbar -> move || {
        let Some(compiled) = row.with(|row| match row {
            Some(Row { kind: RowKind::Component { compiled, .. }, .. }) => Some(*compiled),
            _ => None,
        }) else {
            return;
        };
        if let Some(edit) = hotbar.read(|hotbar| hotbar.root().unpin(compiled)) {
            hotbar.operate(edit);
        }
    });
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
            <Spacer @sizing={indent} />
            <Show condition={is_folder.clone()}>
                <Icon glyph={ICON_FOLDER.to_owned()} color={theme.text_muted.clone()} />
            </Show>
            <Show condition={is_folder}>
                <Body content={folder} />
            </Show>
            <Show condition={is_component.clone()}>
                <BlockLink
                    editor={editor}
                    block={component}
                    fallback={name}
                    @test_id={format!("hotbar.slot.{index}")}
                />
            </Show>
            <Caption content={note} />
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Show condition={is_component}>
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
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
                block: Some(ChildTarget::new(*compiled, CompiledLogic::TYPE_ID)),
            },
        };
        rows.push(Row { depth, kind });
        flatten(&slot.slots, depth + 1, rows);
    }
}
