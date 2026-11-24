use beui::Overflow;
use std::collections::HashSet;

use be_sdk_beui::BlockParent;
use be_sdk_beui::be_block::{WORKSPACE_TYPE, settings::Settings};
use be_sdk_beui::be_block_content::{BlockMetadata, Root};
use be_sdk_beui::be_sdk_ui::BlockTypes;
use be_sdk_beui::beui::clipboard::formats;
use be_sdk_beui::beui::icons::{Glyph, ICON_MOVE, ICON_SETTINGS, ICON_SHARE_2, ICON_SPARKLES};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{ClipItem, Color32, NodeId, Rect};
use be_sdk_beui::{AccountRole, BlockFilter, Drag, Editor, RoleWatch, phone_layout};
use be_sdk_database::tags::{ShownTag, TagChip};
use uuid::Uuid;

use super::hits::{ContentHits, content_hits};
use super::menu::{RowMenu, RowTools, paste_into};
use super::rename::Renaming;
use super::rows::{Inspection, Row, RowKey, Tree as FileTree, access_hint, access_marker};
use super::toolbar::{Adding, FilesToolbar};
use super::zoom::{Zoom, zooms};
use be_sdk_beui::beui::TextAlign;
use be_sdk_beui::beui::styled::Span;
use be_sdk_beui::block_menu::Exporter;
use beui::{Align, Direction};

pub(crate) const PADDING: Space = Space::Md;
const ROW_SPACING: Space = Space::Sm;
const INSPECT_WIDTH: Size = Size::Dialog;
const FIELD_SPACING: Space = Space::Xs2;
const SETTINGS_TYPE: Uuid = <Settings as Root>::CONTENT_TYPE;

#[component]
pub fn FileTreeEditor(editor: Editor) -> NodeId {
    let tree = FileTree::watch(editor);
    let zoom = Zoom::new(editor, tree);
    let keys = zoom.keys();
    let renaming = Renaming::new(editor, tree, keys);
    let tree_ref = ComponentRef::new();
    let picker = picker(editor, tree, renaming, tree_ref);
    let exporter = Exporter::new(editor);
    let held: Held = store(std::cell::RefCell::new(None));
    let inspecting = create_signal(None::<Inspection>);
    let inspected_from = NodeRef::new();
    let tools = RowTools {
        editor,
        tree,
        picker,
        exporter,
        inspect: inspecting,
        inspected_from,
        zoom,
        renaming,
        tree_api: tree_ref,
    };
    let rows = tree.rows();
    let focused_block = editor.focused_block();
    let focused = create_memo(move || focused_block.with(focused_key));

    let landing = create_signal(None::<(RowKey, bool)>);
    let drag = editor.drag();
    let arriving = create_memo(move || landing.get());

    let item = move |key: RowKey| {
        let row = rows.with(|rows| rows.iter().find(|row| row.key == key).cloned());
        TreeItem {
            label: row
                .as_ref()
                .map(|row| row.label.clone())
                .unwrap_or_default(),
            depth: zoom.depth(row.as_ref().map_or(0, |row| row.depth)),
            expandable: row.as_ref().is_some_and(|row| row.expandable),
            expanded: row.as_ref().is_some_and(|row| row.expanded),
        }
    };

    let open = move |key: RowKey| {
        let Some(row) = rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()) else {
            return;
        };
        if zooms(&row) {
            zoom.enter(key);
            return;
        }
        let (Some(id), true) = (row.id, row.access.can_view()) else {
            return;
        };
        match row.container {
            Some(container) => editor.host().open_block_via(id, row.block_type, container),
            None => editor.host().open_block(id, row.block_type),
        }
    };
    let expand = move |(key, expanded): (RowKey, bool)| {
        let id = match &key {
            RowKey::Block(path) => path.last().copied(),
            RowKey::Shared
            | RowKey::Orphans
            | RowKey::System
            | RowKey::Unfiled
            | RowKey::Note(_)
            | RowKey::SharedNote
            | RowKey::OrphansNote
            | RowKey::SystemNote
            | RowKey::UnfiledNote
            | RowKey::Pinned(_) => None,
        };
        tree.toggle(&key, id, !expanded);
    };

    let reveal = move |key: RowKey| {
        let focused = editor.host().focused_block();
        zoom.leave_for(&key);
        tree.show_orphans();
        if focused
            .block_id
            .and_then(|id| editor.blocks().info(id))
            .is_some_and(|info| info.parent == Some(BlockParent::Unfiled))
        {
            tree.show_unfiled();
        }
        tree.reveal_system(focused.via.last().copied().or(focused.block_id));
        tree.expand(focused.via.iter().rev().copied());
    };

    let outline = move |key: RowKey| {
        let (landed, accepts) = arriving.get()?;
        (landed == key).then_some(match accepts {
            true => ColorRole::Accent,
            false => ColorRole::Danger,
        })
    };
    let start = move |key: RowKey| {
        let carried = rows.with_untracked(|rows| {
            let row = rows.iter().find(|row| row.key == key)?;
            Some(Carried {
                id: row.id?,
                block_type: row.block_type,
                source: row.source,
                is_reference: row.is_reference,
                key: row.key.clone(),
                can_reorder: row.can_reorder,
            })
        });
        let Some(carried) = carried else {
            return;
        };
        let (id, block_type) = (carried.id, carried.block_type);
        held.with(|held| *held.borrow_mut() = Some(carried));
        editor.host().drag_block(id, block_type);
    };

    let narrow = phone_layout();
    let chrome = editor.chrome_shown();
    let focus = zoom.focus();
    let query = create_signal(String::new());
    let filter = create_memo(move || query.get().trim().to_lowercase());
    let filtering = create_memo(move || !filter.with(String::is_empty));
    let found = create_memo(move || filter.with(|filter| filtered(rows, keys, filter)));
    let shown_keys = create_memo(move || found.get().keys);
    let content = content_hits(editor, filter);
    let count = create_memo(move || {
        let hits = found.with(|found| found.hits);
        let in_content = content.with(Vec::len);
        match (hits, in_content) {
            (0, 0) => format!("Nothing matches \u{201c}{}\u{201d}", query.get().trim()),
            (0, 1) => "1 match in content".to_owned(),
            (0, in_content) => format!("{in_content} matches in content"),
            (1, _) => "1 match".to_owned(),
            (hits, _) => format!("{hits} matches"),
        }
    });
    let adding = Adding {
        editor,
        tree,
        zoom,
        picker,
        renaming,
        query,
    };
    let content_node = NodeRef::new();
    editor.content(&content_node);
    create_effect(move || {
        let landed = arrival(editor, held, rows, tree_ref, drag);
        landing.set(landed);
    });
    view! {
        <List spacing=Space::None>
            <FilesToolbar adding shown={chrome} />
            <ListItem grow=1.0>
                <Frame @node_ref={&content_node}>
                    <BackSlide
                        enabled={move || narrow.get() && focus.with(Option::is_some)}
                        on_back={move || zoom.up()}
                    >
                        <Frame fill=ColorRole::Background>
                            <List spacing=Space::None>
                                <Show value={filtering}>
                                    <Frame
                                        padding={Padding::ZERO.top(PADDING).left(PADDING).right(PADDING)}
                                    >
                                        <Text
                                            style=TextStyle::CAPTION
                                            @test_id={"file-tree.matches"}
                                        >
                                            {count}
                                        </Text>
                                    </Frame>
                                    <ContentHits editor hits={content} />
                                </Show>
                                <ListItem grow=1.0>
                                    <Tree
                                        api=tree_ref
                                        keys={shown_keys}
                                        item={item}
                                        selected={focused}
                                        ancestors={move |key: RowKey| ancestors(&key)}
                                        spacing=Space::Xs2
                                        padding=PADDING
                                        row_test_id={move |key: RowKey| row_test_id(&key)}
                                        reveal_test_id={"file-tree.reveal".to_owned()}
                                        outline={outline}
                                        menu={move |key: RowKey| view! {
                                            <RowMenu tools key />
                                        }}
                                        editing={renaming.editing()}
                                        reorder=true
                                        pastes={move |key: RowKey| {
                                            let folder = rows.with_untracked(|rows| {
                                                rows.iter().any(|row| row.key == key && row.can_add)
                                            });
                                            match folder {
                                                true => vec![formats::BLOCKS.to_owned()],
                                                false => Vec::new(),
                                            }
                                        }}
                                        on_paste={move |(key, pasted): (RowKey, ClipItem)| {
                                            let row = tree.rows().with_untracked(|rows| {
                                                rows.iter().find(|row| row.key == key).cloned()
                                            });
                                            paste_into(editor, row, pasted)
                                        }}
                                        on_select={open}
                                        on_expand={expand}
                                        on_reveal={reveal}
                                        on_drag_start={start}
                                        on_edit={move |key: RowKey| renaming.start(key)}
                                        on_rename={move |(key, name): (RowKey, String)| renaming.rename(&key, &name)}
                                        on_cancel_edit={move |key: RowKey| renaming.finish(&key)}
                                        leading={move |face: TreeRowFace<RowKey>| view! {
                                            <RowGlyph row={tree.row(face.key)} />
                                        }}
                                    >|face: TreeRowFace<RowKey>| {
                                        let row = tree.row(face.key.clone());
                                        let workspace = row.get_untracked().is_some_and(|row| row.is_workspace());
                                        if workspace {
                                            return view! {
                                                <SectionRow
                                                    editor={editor}
                                                    row={row}
                                                    face={face}
                                                    filter={filter}
                                                />
                                            };
                                        }
                                        view! {
                                            <TreeRow
                                                editor={editor}
                                                row={row}
                                                face={face}
                                                filter={filter}
                                            />
                                        }
                                    }</Tree>
                                </ListItem>
                            </List>
                        </Frame>
                    </BackSlide>
                </Frame>
            </ListItem>
            <Inspector inspecting={inspecting} opened_from={inspected_from} />
        </List>
    }
}

#[component]
fn Inspector(inspecting: Signal<Option<Inspection>>, opened_from: NodeRef) -> NodeId {
    let field = |read: fn(&Inspection) -> &String| {
        create_memo(move || {
            inspecting.with(|shown| shown.as_ref().map(read).cloned().unwrap_or_default())
        })
    };
    let name = field(|shown| &shown.name);
    let id = field(|shown| &shown.id);
    let block_type = field(|shown| &shown.block_type);
    let author = field(|shown| &shown.author);
    let parent = field(|shown| &shown.parent);
    let references = field(|shown| &shown.references);
    let access = field(|shown| &shown.access);
    let generated = field(|shown| &shown.generated);
    let shown_as = field(|shown| &shown.shown_as);
    let size = field(|shown| &shown.size);
    let dismiss = move || inspecting.set(None);
    view! {
        <Dialog
            open={move || inspecting.with(Option::is_some)}
            title="Inspect"
            width=INSPECT_WIDTH
            anchor={DialogAnchor::Control(opened_from)}
            id="file-tree.inspect"
            on_dismiss={dismiss}
        >
            <List spacing=ROW_SPACING>
                <Field label="Name" value={name} named="name" />
                <Field label="ID" value={id} named="id" />
                <Field label="Type" value={block_type} named="type" />
                <Field label="Author" value={author} named="author" />
                <Field label="Parent" value={parent} named="parent" />
                <Field label="References" value={references} named="references" />
                <Field label="Your access" value={access} named="access" />
                <Field label="Generated" value={generated} named="generated" />
                <Field label="Shown here as" value={shown_as} named="shown-as" />
                <Field label="Size" value={size} named="size" />
            </List>
        </Dialog>
    }
}

#[component]
fn Field(label: Prop<String>, value: Memo<String>, named: String) -> NodeId {
    view! {
        <List spacing=FIELD_SPACING>
            <Text style=TextStyle::CAPTION>{label}</Text>
            <Text
                style=TextStyle::PARAGRAPH
                color=TextColor::Default
                @test_id={format!("file-tree.inspect.{named}")}
            >
                {value}
            </Text>
        </List>
    }
}

pub(crate) fn new_workspace(editor: Editor, renaming: Renaming) {
    let id = editor.blocks().create_with(
        WORKSPACE_TYPE,
        None,
        BlockParent::Root,
        BlockMetadata::named("New workspace"),
    );
    renaming.rename_when_shown(id, None);
}

fn focused_key(focused: &be_sdk_beui::FocusedBlock) -> Option<RowKey> {
    let id = focused.block_id?;
    let mut path: Vec<Uuid> = focused.via.iter().rev().copied().collect();
    path.push(id);
    Some(RowKey::Block(path))
}

fn ancestors(key: &RowKey) -> Vec<RowKey> {
    match key {
        RowKey::Block(path) => (1..path.len())
            .map(|length| RowKey::Block(path[..length].to_vec()))
            .collect(),
        RowKey::Shared
        | RowKey::Orphans
        | RowKey::System
        | RowKey::Unfiled
        | RowKey::Note(_)
        | RowKey::SharedNote
        | RowKey::OrphansNote
        | RowKey::SystemNote
        | RowKey::UnfiledNote
        | RowKey::Pinned(_) => Vec::new(),
    }
}

pub(crate) fn row_test_id(key: &RowKey) -> String {
    let named = match key {
        RowKey::Block(path) => path
            .last()
            .map_or_else(|| "block".to_owned(), Uuid::to_string),
        RowKey::Shared => "shared".to_owned(),
        RowKey::Orphans => "orphans".to_owned(),
        RowKey::System => "system".to_owned(),
        RowKey::Unfiled => "unfiled".to_owned(),
        RowKey::Note(path) => format!("note.{}", path.len()),
        RowKey::SharedNote => "note.shared".to_owned(),
        RowKey::OrphansNote => "note.orphans".to_owned(),
        RowKey::SystemNote => "note.system".to_owned(),
        RowKey::UnfiledNote => "note.unfiled".to_owned(),
        RowKey::Pinned(path) => match path.get(1) {
            Some(id) => format!("pinned.{id}"),
            None => "pinned".to_owned(),
        },
    };
    format!("file-tree.{named}")
}

#[component]
fn TreeRow(
    editor: Editor,
    row: Memo<Option<Row>>,
    face: TreeRowFace<RowKey>,
    filter: Memo<String>,
) -> NodeId {
    let row_focused = face.focused;
    let label = create_memo(move || row.get().map(|row| row.label).unwrap_or_default());
    let (before, hit, after) = highlighted(label, filter);
    let tag_keys = create_memo(move || {
        row.get()
            .map(|row| row.tags.iter().map(|tag| tag.id).collect::<Vec<_>>())
            .unwrap_or_default()
    });
    let access = create_memo(move || {
        row.get()
            .filter(|row| row.id.is_some())
            .and_then(|row| access_marker(row.access))
    });
    let access_label = create_memo(move || {
        row.get()
            .map(|row| access_hint(row.access))
            .unwrap_or_default()
            .to_owned()
    });
    let glyph_size = glyph_size();
    let (detail, detailed, inline) = detail(editor, row);
    let beside = create_memo(move || match inline.get() {
        true => format!("  {}", detail.get()),
        false => String::new(),
    });
    let unread = create_memo(move || row.with(|row| row.as_ref().map_or(0, |row| row.unread)));
    let unread_test_id = format!("{}.unread", row_test_id(&face.key));
    let generated_color = TextColor::Muted;
    let access_color = TextColor::Muted;
    let muted = move || {
        row.with(|row| {
            row.as_ref()
                .is_none_or(|row| row.automatic || row.id.is_none() || row.marked_deleted)
        })
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <ListItem grow=1.0>
                <List spacing=Space::None>
                    <Text
                        color={move || match muted() {
                            true => TextColor::Muted,
                            false => TextColor::Default,
                        }}
                        weight={move || match row.with(|row| row.as_ref().is_some_and(|row| row.section)) {
                            true => Weight::Bold,
                            false => Weight::Normal,
                        }}
                        overflow=Overflow::Ellipsis
                    >
                        {before}
                        <Span background=ColorRole::AccentSoft>{hit}</Span>
                        {after}
                        <Span size=TextSize::Sm color=TextColor::Muted>{beside}</Span>
                    </Text>
                    <Show value={detailed}>
                        <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{detail}</Text>
                    </Show>
                </List>
            </ListItem>
            <ForEach value={tag_keys}>|tag: Uuid| {
                let shown = create_memo(move || {
                    row.get()
                        .and_then(|row| row.tags.into_iter().find(|held| held.id == tag))
                        .unwrap_or(ShownTag { id: tag, name: String::new(), color: Default::default() })
                });
                view! {
                    <TagChip tag={shown} size=TextSize::Xs />
                }
            }</ForEach>
            <Show
                value={move || row.with(|row| row.as_ref().is_some_and(|row| row.dynamic_artifact))}
            >
                <Tooltip label="Generated from another block">
                    <Text size={glyph_size} color={generated_color} align=TextAlign::Center>
                        <Icon glyph=ICON_SPARKLES />
                    </Text>
                </Tooltip>
            </Show>
            <Show value={move || unread.get() > 0}>
                <Tooltip
                    label={move || match unread.get() {
                        1 => "1 unread".to_owned(),
                        count => format!("{count} unread"),
                    }}
                    @test_id={unread_test_id.clone()}
                >
                    <CountBadge count={unread} />
                </Tooltip>
            </Show>
            <Show value={move || access.with(Option::is_some)}>
                <Tooltip label={access_label}>
                    <Text size={glyph_size} color={access_color} align=TextAlign::Center>
                        <Icon glyph={access} />
                    </Text>
                </Tooltip>
            </Show>
            <Show value={move || row.with(|row| row.as_ref().is_some_and(has_actions))}>
                <RowActions
                    editor={editor}
                    row={row}
                    key={face.key.clone()}
                    row_focused={row_focused}
                />
            </Show>
        </List>
    }
}

#[component]
fn RowGlyph(row: Memo<Option<Row>>) -> Children<ListItem> {
    let glyph = create_memo(move || row.get().and_then(|row| row.glyph));
    let glyph_size = glyph_size();
    Children::from([into_segment(view! {
        <Show value={move || glyph.with(Option::is_some)}>
            <Text
                size={glyph_size}
                color={move || {
                    row.with(|row| row.as_ref().and_then(|row| row.icon_color))
                        .map_or(TextColor::Muted, |[r, g, b]| {
                            TextColor::Custom(Color32::from_rgb(r, g, b).into())
                        })
                }}
                align=TextAlign::Center
            >
                <Icon glyph={glyph} />
            </Text>
        </Show>
    })])
}

#[component]
fn SectionRow(
    editor: Editor,
    row: Memo<Option<Row>>,
    face: TreeRowFace<RowKey>,
    filter: Memo<String>,
) -> NodeId {
    let row_focused = face.focused;
    let label = create_memo(move || row.get().map(|row| row.label).unwrap_or_default());
    let (before, hit, after) = highlighted(label, filter);
    let (detail, detailed, _) = detail(editor, row);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <ListItem grow=1.0>
                <List spacing=Space::None>
                    <Text weight=Weight::Bold overflow=Overflow::Ellipsis>
                        {before}
                        <Span background=ColorRole::AccentSoft>{hit}</Span>
                        {after}
                    </Text>
                    <Show value={detailed}>
                        <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{detail}</Text>
                    </Show>
                </List>
            </ListItem>
            <Show value={move || row.with(|row| row.as_ref().is_some_and(has_actions))}>
                <RowActions
                    editor={editor}
                    row={row}
                    key={face.key.clone()}
                    row_focused={row_focused}
                />
            </Show>
        </List>
    }
}

/// The line under a row's name, and whether it shows there (comfortable density) or, for a
/// row with a detail of its own such as Unfiled's, after the name on one line (compact).
fn detail(editor: Editor, row: Memo<Option<Row>>) -> (Memo<String>, Memo<bool>, Memo<bool>) {
    let density = use_density();
    let detail = create_memo(move || {
        let Some(row) = row.get().filter(|row| row.id.is_some()) else {
            return String::new();
        };
        if let Some(detail) = row.detail {
            return detail;
        }
        let size = row
            .inspection
            .as_ref()
            .map(|inspection| inspection.tree_size.clone())
            .unwrap_or_default();
        let kind = if zooms(&row) || row.expandable {
            let count = row
                .inspection
                .as_ref()
                .map(|inspection| inspection.references.clone())
                .unwrap_or_default();
            match count.as_str() {
                "1" => "1 item".to_owned(),
                count => format!("{count} items"),
            }
        } else {
            editor
                .block_types()
                .display_name(row.block_type)
                .unwrap_or("Block")
                .to_owned()
        };
        match size.is_empty() {
            true => kind,
            false => format!("{kind} · {size}"),
        }
    });
    let own =
        create_memo(move || row.with(|row| row.as_ref().is_some_and(|row| row.detail.is_some())));
    let detailed = create_memo(move || {
        density.get() == Density::Comfortable && !detail.with(String::is_empty)
    });
    let inline = create_memo(move || {
        own.get() && density.get() == Density::Compact && !detail.with(String::is_empty)
    });
    (detail, detailed, inline)
}

fn glyph_size() -> Memo<TextSize> {
    let density = use_density();
    create_memo(move || match density.get() {
        Density::Compact => TextSize::Xs,
        Density::Comfortable => TextSize::Lg,
    })
}

pub(crate) fn add_to_section(
    editor: Editor,
    picker: Picker,
    id: Uuid,
    anchor: impl Into<be_sdk_beui::PickAnchor>,
) {
    picker.open(
        editor,
        id,
        [editor.block_id(), id]
            .into_iter()
            .collect::<HashSet<Uuid>>(),
        anchor,
    );
}

#[component]
pub(crate) fn OpenWith(editor: Editor, row: Memo<Option<Row>>) -> MenuItem {
    let block = create_memo(move || {
        row.get()
            .filter(|row| row.access.can_view())
            .and_then(|row| Some((row.id?, row.block_type)))
    });
    let file_name = create_memo(move || row.get().and_then(|row| row.file_name));
    let editors = create_memo(move || {
        let types = editor.block_types();
        block.get().map_or_else(Vec::new, |(_, block_type)| {
            types
                .editors_for(block_type, file_name.get().as_deref())
                .into_iter()
                .map(|entry| (entry.editor, entry.name.clone()))
                .collect::<Vec<_>>()
        })
    });
    let keys = create_memo(move || {
        editors.with(|editors| editors.iter().map(|(id, _)| *id).collect::<Vec<_>>())
    });
    view! {
        <MenuItem
            label="Open with"
            glyph=ICON_MOVE
            disabled={move || keys.with(Vec::is_empty)}
            @test_id={"file-tree.open-with".to_owned()}
        >
            <ForEach value={keys}>|chosen: Uuid|
                <MenuItem
                    label={move || {
                        editors.with(|editors| {
                            editors
                                .iter()
                                .find(|(id, _)| *id == chosen)
                                .map(|(_, name)| name.clone())
                                .unwrap_or_default()
                        })
                    }}
                    @test_id={format!("file-tree.open-with.{chosen}")}
                    on_click={move || {
                        if let Some((id, block_type)) = block.get_untracked() {
                            editor.host().open_block_with(id, block_type, chosen);
                        }
                    }}
                />
            </ForEach>
        </MenuItem>
    }
}

#[component]
fn RowButton(
    row_focused: ReadSignal<bool>,
    glyph: Glyph,
    label: &'static str,
    named: String,
    on_click: be_sdk_beui::beui::reactive::Callback<()>,
) -> NodeId {
    let focused = create_signal(false);
    view! {
        <Button
            @test_id={named}
            icon_only=true
            glyph={glyph}
            label={label}
            variant=ButtonVariant::Ghost
            size=ButtonSize::Small
            tab_stop={move || row_focused.get() || focused.get()}
            capture_presses=true
            on_focus_change={focused}
            on_click={on_click}
        />
    }
}

fn has_actions(row: &Row) -> bool {
    (row.sets_access && row.access.can_manage()) || row.settings.is_some()
}

#[component]
fn RowActions(
    editor: Editor,
    row: Memo<Option<Row>>,
    key: RowKey,
    row_focused: ReadSignal<bool>,
) -> NodeId {
    let prefix = row_test_id(&key);
    let (share_id, settings_id) = (format!("{prefix}.share"), format!("{prefix}.settings"));
    let settings = create_memo(move || row.get().and_then(|row| row.settings));
    let share = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().share_block(id);
        }
    };
    let open_settings = move || {
        if let Some(settings) = settings.get_untracked() {
            editor.host().open_block(settings, SETTINGS_TYPE);
        }
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
            <Show
                value={move || {
                    row.with(|row| {
                        row.as_ref()
                            .is_some_and(|row| row.sets_access && row.access.can_manage())
                    })
                }}
            >
                <RowButton
                    row_focused={row_focused}
                    glyph=ICON_SHARE_2
                    label="Share: access is set here"
                    named={share_id.clone()}
                    on_click={share}
                />
            </Show>
            <Show value={move || settings.with(Option::is_some)}>
                <RowButton
                    row_focused={row_focused}
                    glyph=ICON_SETTINGS
                    label="Settings"
                    named={settings_id.clone()}
                    on_click={open_settings}
                />
            </Show>
        </List>
    }
}

#[derive(Clone)]
pub(crate) struct Carried {
    id: Uuid,
    block_type: Uuid,
    source: BlockParent,
    is_reference: bool,
    key: RowKey,
    can_reorder: bool,
}

pub(crate) type Held = Stored<std::cell::RefCell<Option<Carried>>>;

fn arrival(
    editor: Editor,
    held: Held,
    rows: Memo<Vec<Row>>,
    tree: ComponentRef<TreeApi<RowKey>>,
    drag: ReadSignal<Option<Drag>>,
) -> Option<(RowKey, bool)> {
    let drag = drag.get();
    let tree = tree.try_get()?;
    let carried = held.with(|held| held.borrow().clone())?;
    let Some(drag) = drag.filter(|drag| drag.block_id == carried.id) else {
        tree.drag_over(None, |_| false);
        return None;
    };
    let over =
        rows.with(|rows| tree.drag_over(Some(drag.position), |drop| allows(rows, &carried, drop)));
    let drop = over.as_ref().and_then(|over| over.drop.clone());
    if drag.dropped {
        held.with(|held| held.borrow_mut().take());
        let settles = rows.with_untracked(|rows| land(editor, rows, &carried, drop));
        tree.drag_end(settles);
        return None;
    }
    editor.accept_drag(drop.is_some());
    let over = over?;
    match drop {
        Some(TreeDrop::Into(key)) => Some((key, true)),
        Some(TreeDrop::Among { .. }) => None,
        None => rows
            .with_untracked(|rows| find(rows, &over.row).is_some_and(|row| row.id.is_some()))
            .then_some((over.row, false)),
    }
}

fn find<'a>(rows: &'a [Row], key: &RowKey) -> Option<&'a Row> {
    rows.iter().find(|row| row.key == *key)
}

fn holds(key: &RowKey, id: Uuid) -> bool {
    match key {
        RowKey::Block(path) => path.contains(&id),
        RowKey::Pinned(path) => path.last() == Some(&id),
        RowKey::Shared
        | RowKey::Orphans
        | RowKey::System
        | RowKey::Unfiled
        | RowKey::Note(_)
        | RowKey::SharedNote
        | RowKey::OrphansNote
        | RowKey::SystemNote
        | RowKey::UnfiledNote => false,
    }
}

fn container_key(key: &RowKey) -> Option<RowKey> {
    match key {
        RowKey::Block(path) if path.len() > 1 => {
            Some(RowKey::Block(path[..path.len() - 1].to_vec()))
        }
        _ => None,
    }
}

fn takes(row: &Row, carried: &Carried) -> bool {
    row.id.is_some_and(|id| {
        row.can_add && !holds(&row.key, carried.id) && carried.source != BlockParent::Block(id)
    })
}

fn allows(rows: &[Row], carried: &Carried, drop: &TreeDrop<RowKey>) -> bool {
    match drop {
        TreeDrop::Into(key) => find(rows, key).is_some_and(|row| takes(row, carried)),
        TreeDrop::Among { parent, .. } => {
            let Some(parent) = parent.as_ref().and_then(|parent| find(rows, parent)) else {
                return false;
            };
            let home = container_key(&carried.key).as_ref() == Some(&parent.key);
            carried.can_reorder
                && parent.orders_children
                && matches!(parent.key, RowKey::Block(_))
                && (home || takes(parent, carried))
        }
    }
}

fn land(editor: Editor, rows: &[Row], carried: &Carried, drop: Option<TreeDrop<RowKey>>) -> bool {
    let host = editor.host();
    let parent = match drop {
        None => return false,
        Some(TreeDrop::Into(key)) => {
            if let Some(id) = find(rows, &key).and_then(|row| row.id) {
                host.move_block(
                    carried.id,
                    carried.block_type,
                    carried.source,
                    id,
                    carried.is_reference,
                );
            }
            return false;
        }
        Some(TreeDrop::Among { parent, after }) => parent.map(|parent| (parent, after)),
    };
    let Some((parent, after)) = parent else {
        return false;
    };
    let Some(id) = find(rows, &parent).and_then(|row| row.id) else {
        return false;
    };
    let after = after
        .as_ref()
        .and_then(|after| find(rows, after))
        .filter(|row| matches!(row.key, RowKey::Block(_)))
        .and_then(|row| row.id);
    if container_key(&carried.key) == Some(parent.clone()) {
        let siblings: Vec<Uuid> = rows
            .iter()
            .filter(|row| container_key(&row.key).as_ref() == Some(&parent))
            .filter_map(|row| row.id)
            .collect();
        let at = siblings.iter().position(|sibling| *sibling == carried.id);
        let previous = at.and_then(|at| at.checked_sub(1)).map(|at| siblings[at]);
        if after == Some(carried.id) || after == previous {
            return false;
        }
    } else {
        host.move_block(
            carried.id,
            carried.block_type,
            carried.source,
            id,
            carried.is_reference,
        );
    }
    host.reorder_block(carried.id, id, after);
    true
}

fn row_rect(tree: TreeApi<RowKey>, key: &RowKey) -> Option<Rect> {
    Some(measure(tree.row_node(key)?).get_untracked()?.rect)
}

#[derive(Clone, Copy)]
pub(crate) struct Picker {
    inner: Stored<PickerInner>,
}

type Picked = std::rc::Rc<dyn Fn(be_sdk_beui::PickedBlock, Uuid)>;

struct PickerInner {
    picked: Picked,
    tree: ComponentRef<TreeApi<RowKey>>,
}

impl Picker {
    pub(crate) fn open(
        &self,
        editor: Editor,
        parent: Uuid,
        excluded: HashSet<Uuid>,
        anchor: impl Into<be_sdk_beui::PickAnchor>,
    ) {
        let picked = std::rc::Rc::clone(&self.inner.value().picked);
        let place = BlockParent::Block(parent);
        editor.pick_block(
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: excluded.into_iter().map(Uuid::into_bytes).collect(),
                templates: false,
                place: Some(place.encode()),
            },
            anchor,
            move |block| picked(block, parent),
        );
    }

    pub(crate) fn row_anchor(&self, key: &RowKey) -> Option<Rect> {
        let tree = self.inner.try_value()?.tree.try_get()?;
        row_rect(tree, key)
    }
}

fn picker(
    editor: Editor,
    tree: FileTree,
    renaming: Renaming,
    tree_ref: ComponentRef<TreeApi<RowKey>>,
) -> Picker {
    let host = editor.host();
    // On a phone the block opens in the files' place, so its row cannot be renamed where it
    // shows: a name field left open there would come back with the files and take their first
    // tap. It is named from its own title instead.
    let phone = phone_layout();
    let picked: Picked = std::rc::Rc::new(move |picked, parent| {
        let block_type = Uuid::from_bytes(picked.block_type);
        tree.remember(picked.id(), block_type);
        if !picked.named && !phone.get_untracked() {
            renaming.rename_when_shown(picked.id(), Some(parent));
        }
        if !picked.placed {
            host.place_block(picked.id(), block_type, parent, picked.linked);
        }
        host.open_block_via(picked.id(), block_type, parent);
    });
    Picker {
        inner: store(PickerInner {
            picked,
            tree: tree_ref,
        }),
    }
}

pub(crate) fn leavable(editor: Editor, row: Memo<Option<Row>>) -> Memo<bool> {
    let role = editor.watch::<RoleWatch>();
    create_memo(move || {
        role.get() != Some(AccountRole::Administrator) && row.get().is_some_and(|row| row.can_leave)
    })
}

#[derive(Clone, PartialEq)]
struct Found {
    keys: Vec<RowKey>,
    hits: usize,
}

fn filtered(rows: Memo<Vec<Row>>, keys: Memo<Vec<RowKey>>, filter: &str) -> Found {
    let keys = keys.get();
    if filter.is_empty() {
        return Found { keys, hits: 0 };
    }
    rows.with(|rows| {
        let shown: HashSet<&RowKey> = keys.iter().collect();
        let mut kept = HashSet::new();
        let mut above: Vec<(usize, &RowKey)> = Vec::new();
        let mut hits = 0;
        for row in rows.iter().filter(|row| shown.contains(&row.key)) {
            while above.last().is_some_and(|(depth, _)| *depth >= row.depth) {
                above.pop();
            }
            if row.id.is_some() || row.section {
                if row.label.to_lowercase().contains(filter) {
                    hits += 1;
                    kept.insert(&row.key);
                    kept.extend(above.iter().map(|(_, key)| *key));
                }
            }
            above.push((row.depth, &row.key));
        }
        Found {
            keys: keys
                .iter()
                .filter(|key| kept.contains(key))
                .cloned()
                .collect(),
            hits,
        }
    })
}

fn matched(label: &str, filter: &str) -> Option<(usize, usize)> {
    if filter.is_empty() || label.to_lowercase().len() != label.len() {
        return None;
    }
    let start = label.to_lowercase().find(filter)?;
    let end = start + filter.len();
    (label.is_char_boundary(start) && label.is_char_boundary(end)).then_some((start, end))
}

fn highlighted(
    label: Memo<String>,
    filter: Memo<String>,
) -> (Memo<String>, Memo<String>, Memo<String>) {
    let parts = create_memo(move || {
        let label = label.get();
        match filter.with(|filter| matched(&label, filter)) {
            Some((start, end)) => (
                label[..start].to_owned(),
                label[start..end].to_owned(),
                label[end..].to_owned(),
            ),
            None => (label, String::new(), String::new()),
        }
    });
    (
        create_memo(move || parts.get().0),
        create_memo(move || parts.get().1),
        create_memo(move || parts.get().2),
    )
}
