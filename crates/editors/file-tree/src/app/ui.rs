use beui::Overflow;
use std::collections::HashSet;

use block_editor_beui::BlockParent;
use block_editor_beui::be_block::{BlockMetadata, Root, WORKSPACE_TYPE, settings::Settings};
use block_editor_beui::beui::icons::{
    Glyph, ICON_DOWNLOAD, ICON_MOVE, ICON_SETTINGS, ICON_SHARE_2, ICON_SPARKLES,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, NodeId, Rect};
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{AdministratorWatch, BlockFilter, Drag, Editor, phone_layout};
use block_editor_database::tags::{ShownTag, TagChip};
use uuid::Uuid;

use super::export::{Export, ExportFormat, Exporter};
use super::menu::{RowMenu, RowTools};
use super::rename::Renaming;
use super::rows::{Inspection, Row, RowKey, Tree as FileTree, access_hint, access_marker};
use super::toolbar::{Adding, FilesToolbar};
use super::zoom::{Zoom, zooms};
use beui::{Align, Direction, ItemSize};
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::styled::Span;

const PADDING: Space = Space::Md;
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
    let exported = exporter.error();
    let held: Held = store(std::cell::Cell::new(None));
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
            | RowKey::Note(_)
            | RowKey::SharedNote
            | RowKey::OrphansNote
            | RowKey::SystemNote
            | RowKey::Pinned(_) => None,
        };
        tree.toggle(&key, id, !expanded);
    };

    let reveal = move |key: RowKey| {
        let focused = editor.host().focused_block();
        zoom.leave_for(&key);
        tree.show_orphans();
        tree.reveal_system(focused.via.last().copied().or(focused.block_id));
        tree.expand(focused.via.iter().rev().copied());
    };
    let picked = picker.error();
    let failure = create_memo(move || picked.get().or_else(|| exported.get()));
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());

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
            })
        });
        let Some(carried) = carried else {
            return;
        };
        held.with(|held| held.set(Some(carried)));
        editor.host().drag_block(carried.id, carried.block_type);
    };

    let narrow = phone_layout();
    let chrome = editor.chrome_shown();
    let toolbar = create_memo(move || chrome.get());
    let focus = zoom.focus();
    let zoomed = create_memo(move || focus.with(Option::is_some));
    let swipes = create_memo(move || narrow.get() && zoomed.get());
    let query = create_signal(String::new());
    let filter = create_memo(move || query.get().trim().to_lowercase());
    let filtering = create_memo(move || !filter.with(String::is_empty));
    let found = create_memo(move || filter.with(|filter| filtered(rows, keys, filter)));
    let shown_keys = create_memo(move || found.get().keys);
    let empty = create_memo(move || zoomed.get() && !filtering.get() && keys.with(Vec::is_empty));
    let count = create_memo(move || {
        let hits = found.with(|found| found.hits);
        match hits {
            0 => format!("Nothing matches \u{201c}{}\u{201d}", query.get().trim()),
            1 => "1 match".to_owned(),
            hits => format!("{hits} matches"),
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
    let content = NodeRef::new();
    editor.content(&content);
    create_effect(move || {
        let landed = arrival(editor, held, rows, tree_ref, drag);
        landing.set(landed);
    });
    view! {
        <List spacing=Space::None>
            <FilesToolbar adding shown={toolbar} />
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <BackSlide enabled={swipes} on_back={move || zoom.up()}>
                    <Frame fill=ColorRole::Background>
                        <List spacing=Space::None>
                            <Show condition={filtering}>
                                <Frame
                                    padding={Padding::ZERO.top(PADDING).left(PADDING).right(PADDING)}
                                >
                                    <Text style=TextStyle::CAPTION @test_id={"file-tree.matches"}>
                                        {count}
                                    </Text>
                                </Frame>
                            </Show>
                            <Show condition={failed}>
                                <Frame padding=PADDING>
                                    <Text style=TextStyle::CAPTION color=TextColor::Danger>
                                        {reason}
                                    </Text>
                                </Frame>
                            </Show>
                            <Show condition={empty}>
                                <Frame padding=PADDING @test_id={"file-tree.empty"}>
                                    <Text style=TextStyle::CAPTION>"Nothing here yet"</Text>
                                </Frame>
                            </Show>
                            <Tree
                                @sizing=ItemSize::Percent(100.0)
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
                        </List>
                    </Frame>
                </BackSlide>
            </Frame>
            <Inspector inspecting={inspecting} opened_from={inspected_from} />
        </List>
    }
}

#[component]
fn Inspector(inspecting: Signal<Option<Inspection>>, opened_from: NodeRef) -> NodeId {
    let open = create_memo(move || inspecting.get().is_some());
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
    let dismiss = move || inspecting.set(None);
    view! {
        <Dialog
            open={open}
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

fn focused_key(focused: &block_editor_beui::FocusedBlock) -> Option<RowKey> {
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
        | RowKey::Note(_)
        | RowKey::SharedNote
        | RowKey::OrphansNote
        | RowKey::SystemNote
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
        RowKey::Note(path) => format!("note.{}", path.len()),
        RowKey::SharedNote => "note.shared".to_owned(),
        RowKey::OrphansNote => "note.orphans".to_owned(),
        RowKey::SystemNote => "note.system".to_owned(),
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
    let generated = create_memo(move || row.get().is_some_and(|row| row.dynamic_artifact));
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
    let restricted = create_memo(move || access.get().is_some());
    let actions = create_memo(move || row.get().is_some_and(|row| has_actions(&row)));
    let muted = create_memo(move || {
        row.get()
            .is_none_or(|row| row.automatic || row.id.is_none() || row.marked_deleted)
    });
    let weight = create_memo(move || match row.get().is_some_and(|row| row.section) {
        true => Weight::Bold,
        false => Weight::Normal,
    });
    let glyph_size = glyph_size();
    let (detail, detailed) = detail(editor, row);
    let generated_color = TextColor::Muted;
    let access_color = TextColor::Muted;
    let color = create_memo(move || match muted.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                <Text color={color} weight={weight} overflow=Overflow::Ellipsis>
                    {before}
                    <Span background=ColorRole::AccentSoft>{hit}</Span>
                    {after}
                </Text>
                <Show condition={detailed}>
                    <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{detail}</Text>
                </Show>
            </List>
            <ForEach keys={tag_keys}>|tag: Uuid| {
                let shown = create_memo(move || {
                    row.get()
                        .and_then(|row| row.tags.into_iter().find(|held| held.id == tag))
                        .unwrap_or(ShownTag { id: tag, name: String::new(), color: Default::default() })
                });
                view! {
                    <TagChip tag={shown} size=TextSize::Xs />
                }
            }</ForEach>
            <Show condition={generated}>
                <Tooltip label="Generated from another block">
                    <Text size={glyph_size} color={generated_color} align=TextAlign::Center>
                        <Icon glyph=ICON_SPARKLES />
                    </Text>
                </Tooltip>
            </Show>
            <Show condition={restricted}>
                <Tooltip label={access_label}>
                    <Text size={glyph_size} color={access_color} align=TextAlign::Center>
                        <Icon glyph={access} />
                    </Text>
                </Tooltip>
            </Show>
            <Show condition={actions}>
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
fn RowGlyph(row: Memo<Option<Row>>) -> Children<ListChild> {
    let glyph = create_memo(move || row.get().and_then(|row| row.glyph));
    let has_glyph = create_memo(move || glyph.get().is_some());
    let glyph_size = glyph_size();
    let glyph_color = create_memo(move || {
        row.get()
            .and_then(|row| row.icon_color)
            .map_or(TextColor::Muted, |[r, g, b]| {
                TextColor::Custom(Color32::from_rgb(r, g, b).into())
            })
    });
    Children::from([into_segment(view! {
        <Show condition={has_glyph}>
            <Text size={glyph_size} color={glyph_color} align=TextAlign::Center>
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
    let actions = create_memo(move || row.get().is_some_and(|row| has_actions(&row)));
    let (detail, detailed) = detail(editor, row);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                <Text weight=Weight::Bold overflow=Overflow::Ellipsis>
                    {before}
                    <Span background=ColorRole::AccentSoft>{hit}</Span>
                    {after}
                </Text>
                <Show condition={detailed}>
                    <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{detail}</Text>
                </Show>
            </List>
            <Show condition={actions}>
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

fn detail(editor: Editor, row: Memo<Option<Row>>) -> (Memo<String>, Memo<bool>) {
    let density = use_density();
    let detail = create_memo(move || {
        let Some(row) = row.get().filter(|row| row.id.is_some()) else {
            return String::new();
        };
        if zooms(&row) || row.expandable {
            let count = row
                .inspection
                .as_ref()
                .map(|inspection| inspection.references.clone())
                .unwrap_or_default();
            return match count.as_str() {
                "1" => "1 item".to_owned(),
                count => format!("{count} items"),
            };
        }
        editor
            .block_types()
            .display_name(row.block_type)
            .unwrap_or("Block")
            .to_owned()
    });
    let detailed = create_memo(move || {
        density.get() == Density::Comfortable && !detail.with(String::is_empty)
    });
    (detail, detailed)
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
    anchor: impl Into<block_editor_beui::PickAnchor>,
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
    let none = create_memo(move || keys.with(Vec::is_empty));
    view! {
        <MenuItem
            label="Open with"
            glyph=ICON_MOVE
            disabled={none}
            row_test_id={"file-tree.open-with".to_owned()}
        >
            <ForEach keys={keys}>|chosen: Uuid| {
                let label = create_memo(move || {
                    editors.with(|editors| {
                        editors
                            .iter()
                            .find(|(id, _)| *id == chosen)
                            .map(|(_, name)| name.clone())
                            .unwrap_or_default()
                    })
                });
                view! {
                    <MenuItem
                        label={label}
                        row_test_id={format!("file-tree.open-with.{chosen}")}
                        on_click={move || {
                            if let Some((id, block_type)) = block.get_untracked() {
                                editor.host().open_block_with(id, block_type, chosen);
                            }
                        }}
                    />
                }
            }</ForEach>
        </MenuItem>
    }
}

#[component]
pub(crate) fn ExportAs(
    exporter: Exporter,
    row: Memo<Option<Row>>,
    row_test_id: String,
) -> MenuItem {
    let block = create_memo(move || {
        row.get()
            .filter(|row| row.access.can_view())
            .and_then(|row| Some((row.id?, row.block_type)))
    });
    let shown = create_memo(move || {
        block
            .get()
            .map_or_else(Vec::new, |(_, block_type)| exporter.formats(block_type))
    });
    let keys = create_memo(move || {
        shown.with(|shown| shown.iter().map(|(format, _)| *format).collect::<Vec<_>>())
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    let prefix = row_test_id.clone();
    view! {
        <MenuItem label="Export as" glyph=ICON_DOWNLOAD disabled={none} row_test_id={row_test_id}>
            <ForEach keys={keys}>|format: ExportFormat| {
                let label = create_memo(move || {
                    shown.with(|shown| {
                        shown
                            .iter()
                            .find(|(shown, _)| *shown == format)
                            .map(|(_, label)| label.clone())
                            .unwrap_or_default()
                    })
                });
                let named = match format {
                    ExportFormat::Own => format!("{prefix}.own"),
                    ExportFormat::BlockFile => format!("{prefix}.block"),
                };
                view! {
                    <MenuItem
                        label={label}
                        row_test_id={named}
                        on_click={move || {
                            if let Some((id, _)) = block.get_untracked() {
                                exporter.export(Export { id, format });
                            }
                        }}
                    />
                }
            }</ForEach>
        </MenuItem>
    }
}

#[component]
fn RowButton(
    row_focused: ReadSignal<bool>,
    glyph: Glyph,
    label: Memo<String>,
    named: String,
    on_click: block_editor_beui::beui::reactive::ClickCallback,
) -> NodeId {
    let focused = create_signal(false);
    let reachable = create_memo(move || row_focused.get() || focused.get());
    view! {
        <Button
            @test_id={named}
            icon_only=true
            glyph={glyph}
            label={label}
            variant=ButtonVariant::Ghost
            size=ButtonSize::Small
            tab_stop={reachable}
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
    let shared = create_memo(move || {
        row.get()
            .is_some_and(|row| row.sets_access && row.access.can_manage())
    });
    let settings = create_memo(move || row.get().and_then(|row| row.settings));
    let has_settings = create_memo(move || settings.get().is_some());
    let share_label = create_memo(|| "Share: access is set here".to_owned());
    let settings_label = create_memo(|| "Settings".to_owned());
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
            <Show condition={shared}>
                <RowButton
                    row_focused={row_focused}
                    glyph=ICON_SHARE_2
                    label={share_label}
                    named={share_id.clone()}
                    on_click={share}
                />
            </Show>
            <Show condition={has_settings}>
                <RowButton
                    row_focused={row_focused}
                    glyph=ICON_SETTINGS
                    label={settings_label}
                    named={settings_id.clone()}
                    on_click={open_settings}
                />
            </Show>
        </List>
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Carried {
    id: Uuid,
    block_type: Uuid,
    source: BlockParent,
    is_reference: bool,
}

pub(crate) type Held = Stored<std::cell::Cell<Option<Carried>>>;

fn arrival(
    editor: Editor,
    held: Held,
    rows: Memo<Vec<Row>>,
    tree: ComponentRef<TreeApi<RowKey>>,
    drag: ReadSignal<Option<Drag>>,
) -> Option<(RowKey, bool)> {
    let drag = drag.get()?;
    let carried = held.with(std::cell::Cell::get)?;
    let tree = tree.try_get()?;
    if carried.id != drag.block_id {
        return None;
    }
    let (key, id, can_add) = rows.with(|rows| {
        rows.iter().find_map(|row| {
            let id = row.id?;
            let rect = row_rect(tree, &row.key)?;
            rect.contains(drag.position)
                .then(|| (row.key.clone(), id, row.can_add))
        })
    })?;
    let accepts = can_add && carried.id != id && carried.source != BlockParent::Block(id);
    if drag.dropped {
        held.with(|held| held.set(None));
        if accepts {
            editor.host().move_block(
                carried.id,
                carried.block_type,
                carried.source,
                id,
                carried.is_reference,
            );
        }
        return None;
    }
    editor.accept_drag(accepts);
    Some((key, accepts))
}

fn row_rect(tree: TreeApi<RowKey>, key: &RowKey) -> Option<Rect> {
    Some(measure(tree.row_node(key)?).get_untracked()?.rect)
}

#[derive(Clone, Copy)]
pub(crate) struct Picker {
    inner: Stored<PickerInner>,
    error: Signal<Option<String>>,
}

type Picked = std::rc::Rc<dyn Fn(block_editor_beui::PickedBlock, Uuid)>;

struct PickerInner {
    picked: Picked,
    tree: ComponentRef<TreeApi<RowKey>>,
}

impl Picker {
    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error;
        create_memo(move || error.get())
    }

    pub(crate) fn open(
        &self,
        editor: Editor,
        parent: Uuid,
        excluded: HashSet<Uuid>,
        anchor: impl Into<block_editor_beui::PickAnchor>,
    ) {
        self.error.set(None);
        let picked = std::rc::Rc::clone(&self.inner.value().picked);
        let error = self.error;
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
            move |result| match result {
                Ok(block) => picked(block, parent),
                Err(failure) => error.set(Some(failure.to_string())),
            },
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
    let error = create_signal(None::<String>);
    let host = editor.host();
    let picked: Picked = std::rc::Rc::new(move |picked, parent| {
        let block_type = Uuid::from_bytes(picked.block_type);
        tree.remember(picked.id(), block_type);
        if !picked.linked {
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
        error,
    }
}

pub(crate) fn leavable(editor: Editor, row: Memo<Option<Row>>) -> Memo<bool> {
    let administrator = editor.watch::<AdministratorWatch>();
    create_memo(move || !administrator.get() && row.get().is_some_and(|row| row.can_leave))
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
