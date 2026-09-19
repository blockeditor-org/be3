use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block::Block;
use block_client::block_ref::BlockRef;
use block_client::blocks::compiled_logic::CompiledLogic;
use block_client::blocks::hotbar::{Hotbar, HotbarOperation, HotbarSlot};
use block_client::references::ReferenceResolutionCache;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{ICON_DELETE, ICON_FOLDER};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Scroll, Show, Spacer, clone, component,
    create_memo, create_signal, view,
};
use block_editor_plugin::beui::styled::{Body, Caption, Icon, IconButton, use_theme};
use block_editor_plugin::{BlockLink, BlockProjection, ChildTarget, Editor};
use uuid::Uuid;

const PADDING: f32 = 16.0;
const INDENT: f32 = 16.0;
const ROW_SPACING: f32 = 6.0;

type Pinned = Rc<BlockProjection<Hotbar>>;

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
        compiled: BlockRef,
        block: Option<ChildTarget>,
    },
}

#[component]
pub fn HotbarView(editor: Editor) -> NodeId {
    let hotbar = editor.block::<Hotbar>();
    let pinned = hotbar.project(|hotbar| hotbar.slots().to_vec());
    let slots = create_memo(clone!(pinned -> move || pinned.get()));
    let references = hotbar.project(Hotbar::component_refs);
    let (resolved, set_resolved) = create_signal(HashMap::<BlockRef, Option<Uuid>>::new());
    let cache = RefCell::new(ReferenceResolutionCache::default());
    let client = editor.client().clone();
    let referencing = editor.block_id();
    editor.each_frame(clone!(references -> move || {
        let mut cache = cache.borrow_mut();
        cache.poll();
        let resolved = references.with(|references| {
            references
                .iter()
                .map(|reference| {
                    (*reference, cache.resolve(&client, referencing, *reference))
                })
                .collect()
        });
        set_resolved.set(resolved);
    }));

    let rows = create_memo(clone!(pinned resolved -> move || {
        let mut rows = Vec::new();
        pinned.with(|slots| resolved.with(|resolved| flatten(slots, 0, resolved, &mut rows)));
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
                <Scroll @sizing=ItemSize::Percent(100.0) focus_color={theme.accent.clone()}>
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
                                        slots={slots.clone()}
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
    slots: Memo<Vec<HotbarSlot>>,
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
    let unpin = clone!(row slots hotbar -> move || {
        let Some(compiled) = row.with(|row| match row {
            Some(Row { kind: RowKind::Component { compiled, .. }, .. }) => Some(*compiled),
            _ => None,
        }) else {
            return;
        };
        let remaining = slots.with(|slots| super::without_component(slots, compiled));
        hotbar.operate(HotbarOperation::SetSlots { slots: remaining });
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

fn flatten(
    slots: &[HotbarSlot],
    depth: usize,
    resolved: &HashMap<BlockRef, Option<Uuid>>,
    rows: &mut Vec<Row>,
) {
    for slot in slots {
        let kind = match slot {
            HotbarSlot::Builtin { tool } => RowKind::Note(tool.clone()),
            HotbarSlot::Locked { name } => RowKind::Note(format!("{name} (locked)")),
            HotbarSlot::Folder { name, .. } => RowKind::Folder(name.clone()),
            HotbarSlot::Component { name, compiled } => RowKind::Component {
                name: name.clone(),
                compiled: *compiled,
                block: resolved
                    .get(compiled)
                    .copied()
                    .flatten()
                    .map(|id| ChildTarget::new(id, CompiledLogic::TYPE_ID)),
            },
        };
        rows.push(Row { depth, kind });
        if let HotbarSlot::Folder { slots, .. } = slot {
            flatten(slots, depth + 1, resolved, rows);
        }
    }
}
