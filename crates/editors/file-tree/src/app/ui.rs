use std::collections::HashSet;

use block_editor_beui::BlockParent;
use block_editor_beui::be_block::WorkspaceContent;
use block_editor_beui::beui::accesskit::{Node as AccessNode, Role};
use block_editor_beui::beui::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_ADD, ICON_AUTO_AWESOME, ICON_CLOSE, ICON_DELETE,
    ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_FILE_DOWNLOAD, ICON_INFO, ICON_LINK_OFF, ICON_LOGOUT,
    ICON_MOVE_UP, ICON_NOTE_ADD, ICON_OPEN_WITH, ICON_SHARE, ICON_WORKSPACES,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, ButtonHandle, MenuItem, TreeItem};
use block_editor_beui::beui::{Color32, NodeId, Rect};
use block_editor_beui::{
    Administrator, BlockFilter, BlockPicker, Drag, Editor, Toolbar, phone_layout,
};
use uuid::Uuid;

use super::export::{Export, ExportFormat, Exporter, formats};
use super::phone::PhoneFiles;
use super::rows::{
    Inspection, Row, RowKey, Tree as FileTree, access_hint, access_marker, is_workspace,
};
use block_editor_beui::beui::TextAlign;

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 6.0;
const ADD_WIDTH: f32 = 20.0;
const INSPECT_WIDTH: f32 = 420.0;
const FIELD_SPACING: f32 = 2.0;

#[component]
pub fn FileTreeEditor(editor: Editor) -> NodeId {
    let tree = FileTree::watch(editor);
    let picker = picker(editor, tree);
    let exporter = Exporter::new(editor);
    let exported = exporter.error();
    let held: Held = store(std::cell::Cell::new(None));
    let (inspecting, set_inspecting) = create_signal(None::<Inspection>);
    let inspect = set_inspecting;
    let keys = tree.keys();
    let rows = tree.rows();
    let focused_block = editor.focused_block();
    let focused = create_memo(move || focused_block.with(focused_key));

    let tree_ref = NodeRef::new();
    let (landing, set_landing) = create_signal(None::<(RowKey, bool)>);
    let drag = editor.drag();
    let arriving = create_memo(move || landing.get());

    let item = move |key: RowKey| {
        let row = rows.with(|rows| rows.iter().find(|row| row.key == key).cloned());
        TreeItem {
            label: row
                .as_ref()
                .map(|row| row.label.clone())
                .unwrap_or_default(),
            depth: row.as_ref().map_or(0, |row| row.depth),
            expandable: row.as_ref().is_some_and(|row| row.expandable),
            expanded: row.as_ref().is_some_and(|row| row.expanded),
        }
    };

    let open = move |key: RowKey| {
        let Some(row) = rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()) else {
            return;
        };
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
            | RowKey::Note(_)
            | RowKey::SharedNote
            | RowKey::OrphansNote => None,
        };
        tree.toggle(&key, id, !expanded);
    };

    let reveal = move |_: RowKey| {
        tree.show_orphans();
        tree.expand(editor.host().focused_block().via.iter().rev().copied());
    };
    let app_menu = move || editor.host().show_app_menu(editor.block_id());
    let add_root = move || {
        let Some(files) = tree.my_files() else {
            return;
        };
        picker.open(
            editor,
            Some(files),
            [editor.block_id(), files]
                .into_iter()
                .collect::<HashSet<Uuid>>(),
        );
    };
    let new_workspace = move || new_workspace(editor);
    let picked = picker.error();
    let failure = create_memo(move || picked.get().or_else(|| exported.get()));
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());

    let theme = use_theme();
    let outline = move |key: RowKey| {
        let (landed, accepts) = arriving.get()?;
        (landed == key).then(|| match accepts {
            true => theme.accent.get(),
            false => theme.danger.get(),
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
    let wide = create_memo(move || !narrow.get());
    let chrome = editor.chrome_shown();
    let toolbar = create_memo(move || chrome.get() && wide.get());
    let content = NodeRef::new();
    editor.content(&content);
    let phone_inspect = inspect;
    let covered = create_memo(move || inspecting.get().is_some());
    create_effect(move || {
        let landed = arrival(editor, held, rows, tree_ref, drag);
        set_landing.set(landed);
    });
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=0.0>
                <Toolbar shown={toolbar}>
                    <IconButton
                        glyph={ICON_ADD.to_owned()}
                        label="Add a block to My files"
                        @test_id={"file-tree.add-root"}
                        on_click={add_root}
                    />
                    <IconButton
                        glyph={ICON_WORKSPACES.to_owned()}
                        label="New workspace"
                        @test_id={"file-tree.new-workspace"}
                        on_click={new_workspace}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <IconButton
                        glyph={ICON_ACCOUNT_CIRCLE.to_owned()}
                        label="Account and settings"
                        @test_id={"file-tree.desktop-app-menu"}
                        on_click={app_menu}
                    />
                </Toolbar>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                    <List spacing=0.0>
                        <Show condition={narrow}>
                            <PhoneFiles
                                @sizing=ItemSize::Percent(100.0)
                                editor={editor}
                                tree={tree}
                                picker={picker}
                                exporter={exporter}
                                inspect={phone_inspect}
                                covered={covered}
                            />
                        </Show>
                        <Show condition={failed}>
                            {move || view! {
                                <Frame padding=PADDING>
                                    <Text
                                        style=TextStyle::CAPTION
                                        string={reason}
                                        color=TextColor::Danger
                                    />
                                </Frame>
                            }}
                        </Show>
                        <Show condition={wide}>
                            {move || view! {
                                <Tree
                                    @sizing=ItemSize::Percent(100.0)
                                    @node_ref={&tree_ref}
                                    keys={keys}
                                    item={item}
                                    selected={focused}
                                    ancestors={move |key: RowKey| ancestors(&key)}
                                    spacing=2.0
                                    padding=PADDING
                                    row_test_id={move |key: RowKey| row_test_id(&key)}
                                    reveal_test_id={"file-tree.reveal".to_owned()}
                                    outline={outline}
                                    on_select={open}
                                    on_expand={expand}
                                    on_reveal={reveal}
                                    on_drag_start={start}
                                >
                                    {move |face: TreeRowFace<RowKey>| {
                                        let row = tree.row(face.key.clone());
                                        let workspace = row.get_untracked().is_some_and(|row| {
                                            row.section && is_workspace(row.block_type)
                                        });
                                        if workspace {
                                            return view! {
                                                <SectionRow
                                                    editor={editor}
                                                    picker={picker}
                                                    row={row}
                                                    face={face}
                                                />
                                            };
                                        }
                                        view! {
                                            <TreeRow
                                                editor={editor}
                                                tree={tree}
                                                picker={picker}
                                                exporter={exporter}
                                                inspect={inspect}
                                                row={row}
                                                face={face}
                                            />
                                        }
                                    }}
                                </Tree>
                            }}
                        </Show>
                    </List>
                </Frame>
                <Inspector inspecting={inspecting} set_inspecting={set_inspecting} />
            </List>
        </Frame>
    }
}

#[component]
fn Inspector(
    inspecting: ReadSignal<Option<Inspection>>,
    set_inspecting: WriteSignal<Option<Inspection>>,
) -> NodeId {
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
    let dismiss = move || set_inspecting.set(None);
    let close = move || set_inspecting.set(None);
    view! {
        <Dialog open={open} title="Inspect" width=INSPECT_WIDTH on_dismiss={dismiss}>
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
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::End
                    spacing=ROW_SPACING
                >
                    <Button
                        glyph={ICON_CLOSE.to_owned()}
                        label="Close"
                        variant=ButtonVariant::Secondary
                        @test_id={"file-tree.inspect.close"}
                        on_click={close}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn Field(label: Prop<String>, value: Memo<String>, named: String) -> NodeId {
    view! {
        <List spacing=FIELD_SPACING>
            <Text style=TextStyle::CAPTION string={label} />
            <Text
                style=TextStyle::PARAGRAPH
                string={value}
                color=TextColor::Default
                @test_id={format!("file-tree.inspect.{named}")}
                wrap=true
            />
        </List>
    }
}

pub(crate) fn new_workspace(editor: Editor) {
    let id = editor.blocks().create_named(
        &WorkspaceContent::default(),
        BlockParent::Root,
        "New workspace",
    );
    editor.host().rename_block(id);
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
        | RowKey::Note(_)
        | RowKey::SharedNote
        | RowKey::OrphansNote => Vec::new(),
    }
}

fn row_test_id(key: &RowKey) -> String {
    let named = match key {
        RowKey::Block(path) => path
            .last()
            .map_or_else(|| "block".to_owned(), Uuid::to_string),
        RowKey::Shared => "shared".to_owned(),
        RowKey::Orphans => "orphans".to_owned(),
        RowKey::Note(path) => format!("note.{}", path.len()),
        RowKey::SharedNote => "note.shared".to_owned(),
        RowKey::OrphansNote => "note.orphans".to_owned(),
    };
    format!("file-tree.{named}")
}

#[component]
fn TreeRow(
    editor: Editor,
    tree: FileTree,
    picker: Picker,
    exporter: Exporter,
    inspect: WriteSignal<Option<Inspection>>,
    row: Memo<Option<Row>>,
    face: TreeRowFace<RowKey>,
) -> NodeId {
    let named = format!("{}.add", row_test_id(&face.key));
    let hovered = face.hovered;
    let label = create_memo(move || row.get().map(|row| row.label).unwrap_or_default());
    let glyph = create_memo(move || row.get().map(|row| row.glyph).unwrap_or_default());
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let generated = create_memo(move || row.get().is_some_and(|row| row.dynamic_artifact));
    let access = create_memo(move || {
        row.get()
            .filter(|row| row.id.is_some())
            .and_then(|row| access_marker(row.access))
            .unwrap_or_default()
            .to_owned()
    });
    let access_label = create_memo(move || {
        row.get()
            .map(|row| access_hint(row.access))
            .unwrap_or_default()
            .to_owned()
    });
    let restricted = create_memo(move || !access.get().is_empty());
    let can_add = create_memo(move || row.get().is_some_and(|row| row.can_add));
    let muted = create_memo(move || {
        row.get()
            .is_none_or(|row| row.automatic || row.id.is_none())
    });

    let add_child = move || {
        let Some(id) = row.get_untracked().and_then(|row| row.id) else {
            return;
        };
        picker.open(
            editor,
            Some(id),
            [id].into_iter().collect::<HashSet<Uuid>>(),
        );
    };
    let chose = menu_action(editor, picker, inspect, row);
    let leavable = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let add = create_memo(move || !row.get().is_some_and(|row| row.can_add));
    let edit = create_memo(move || !row.get().is_some_and(|row| row.can_edit));
    let unlinkable = create_memo(move || {
        !row.get()
            .is_some_and(|row| row.is_reference && row.unlink.is_ok())
    });
    let deletable = create_memo(move || !row.get().is_some_and(|row| row.can_delete));
    let delete_label = create_memo(
        move || match row.get().is_some_and(|row| row.is_reference) {
            true => "Remove link".to_owned(),
            false => "Delete".to_owned(),
        },
    );
    let rooted = create_memo(move || edit.get() || !row.get().is_some_and(|row| row.can_delete));
    let orphaned = create_memo(move || {
        edit.get()
            || row
                .get()
                .is_some_and(|row| row.parent == BlockParent::Detached)
    });
    let uninspectable = create_memo(move || !row.get().is_some_and(|row| row.inspection.is_some()));
    let weight = create_memo(move || match row.get().is_some_and(|row| row.section) {
        true => Weight::Bold,
        false => Weight::Normal,
    });
    let rows = tree.rows();
    let destinations = create_memo(move || {
        let here = row.get().and_then(|row| row.container);
        rows.with(|rows| {
            rows.iter()
                .filter(|other| other.section && is_workspace(other.block_type))
                .filter_map(|other| Some((other.id?, other.label.clone())))
                .filter(|(id, _)| Some(*id) != here)
                .collect::<Vec<_>>()
        })
    });
    let destination_keys = create_memo(move || {
        destinations.with(|shown| shown.iter().map(|(id, _)| *id).collect::<Vec<_>>())
    });
    let glyph_color = TextColor::Muted;
    let generated_color = TextColor::Muted;
    let access_color = TextColor::Muted;
    let color = create_memo(move || match muted.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    view! {
        <ContextMenu
            child_size=ItemSize::Percent(100.0)
            items={view! {
                <OpenWith editor={editor} row={row} />
                <MenuItem label="Add" glyph={ICON_NOTE_ADD.to_owned()} disabled={add} />
                <MenuItem
                    label="Move to"
                    glyph={ICON_MOVE_UP.to_owned()}
                    disabled={rooted}
                    row_test_id={"file-tree.move-to".to_owned()}
                >
                    <ForEach keys={destination_keys}>
                        {move |target: Uuid| {
                            let label = create_memo(move || {
                                destinations.with(|shown| {
                                    shown
                                        .iter()
                                        .find(|(id, _)| *id == target)
                                        .map(|(_, name)| name.clone())
                                        .unwrap_or_default()
                                })
                            });
                            view! {
                                <MenuItem
                                    label={label}
                                    row_test_id={format!("file-tree.move-to.{target}")}
                                    on_click={move || {
                                        if let Some(shown) = row.get_untracked()
                                            && let Some(id) = shown.id
                                        {
                                            editor.host().move_block(
                                                id,
                                                shown.block_type,
                                                shown.source,
                                                target,
                                                shown.is_reference,
                                            );
                                        }
                                    }}
                                />
                            }
                        }}
                    </ForEach>
                    <MenuItem
                        label="Recently Deleted"
                        disabled={orphaned}
                        on_click={move || {
                            if let Some(id) = row.get_untracked().and_then(|row| row.id) {
                                tree.blocks().set_parent(id, BlockParent::Detached);
                            }
                        }}
                    />
                </MenuItem>
                <MenuItem
                    label="Rename"
                    glyph={ICON_DRIVE_FILE_RENAME_OUTLINE.to_owned()}
                    disabled={edit}
                />
                <MenuItem label="Share" glyph={ICON_SHARE.to_owned()} disabled={edit} />
                <MenuItem label="Unlink" glyph={ICON_LINK_OFF.to_owned()} disabled={unlinkable} />
                <MenuItem
                    label={delete_label}
                    glyph={ICON_DELETE.to_owned()}
                    disabled={deletable}
                    danger=true
                />
                <MenuItem label="Inspect" glyph={ICON_INFO.to_owned()} disabled={uninspectable} />
                <ExportAs exporter row row_test_id={"file-tree.export".to_owned()} />
                <Show condition={leavable}>
                    <MenuItem
                        label="Leave"
                        glyph={ICON_LOGOUT.to_owned()}
                        danger=true
                        row_test_id={"file-tree.leave".to_owned()}
                        on_click={leave}
                    />
                </Show>
            }}
            on_select={chose}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Show condition={has_glyph}>
                    <Text size=TextSize::Xs color={glyph_color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={label}
                    color={color}
                    weight={weight}
                />
                <Show condition={generated}>
                    {move || view! {
                        <Tooltip label="Generated from another block">
                            <Text size=TextSize::Xs color={generated_color} align=TextAlign::Center>
                                <Icon glyph={ICON_AUTO_AWESOME.to_owned()} />
                            </Text>
                        </Tooltip>
                    }}
                </Show>
                <Show condition={restricted}>
                    {move || view! {
                        <Tooltip label={access_label}>
                            <Text size=TextSize::Xs color={access_color} align=TextAlign::Center>
                                <Icon glyph={access} />
                            </Text>
                        </Tooltip>
                    }}
                </Show>
                <Show condition={can_add}>
                    <AddChild shown={hovered} named={named.clone()} on_click={add_child} />
                </Show>
            </List>
        </ContextMenu>
    }
}

#[component]
fn SectionRow(
    editor: Editor,
    picker: Picker,
    row: Memo<Option<Row>>,
    face: TreeRowFace<RowKey>,
) -> NodeId {
    let named = format!("{}.add", row_test_id(&face.key));
    let hovered = face.hovered;
    let label = create_memo(move || row.get().map(|row| row.label).unwrap_or_default());
    let glyph = create_memo(move || row.get().map(|row| row.glyph).unwrap_or_default());
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let can_add = create_memo(move || row.get().is_some_and(|row| row.can_add));
    let edit = create_memo(move || !row.get().is_some_and(|row| row.can_edit));
    let deletable = create_memo(move || !row.get().is_some_and(|row| row.can_delete));
    let leavable = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let add_child = move || {
        let Some(id) = row.get_untracked().and_then(|row| row.id) else {
            return;
        };
        picker.open(
            editor,
            Some(id),
            [id].into_iter().collect::<HashSet<Uuid>>(),
        );
    };
    let chose = move |path: Vec<usize>| {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        match path.as_slice() {
            [0] => editor.host().rename_block(id),
            [1] => editor.host().share_block(id),
            [2] => editor
                .host()
                .delete_block(id, shown.block_type, shown.source, false),
            _ => {}
        }
    };
    view! {
        <ContextMenu
            child_size=ItemSize::Percent(100.0)
            items={view! {
                <MenuItem
                    label="Rename"
                    glyph={ICON_DRIVE_FILE_RENAME_OUTLINE.to_owned()}
                    disabled={edit}
                    row_test_id={"file-tree.section.rename".to_owned()}
                />
                <MenuItem
                    label="Share"
                    glyph={ICON_SHARE.to_owned()}
                    disabled={edit}
                    row_test_id={"file-tree.section.share".to_owned()}
                />
                <MenuItem
                    label="Delete"
                    glyph={ICON_DELETE.to_owned()}
                    disabled={deletable}
                    danger=true
                    row_test_id={"file-tree.section.delete".to_owned()}
                />
                <Show condition={leavable}>
                    <MenuItem
                        label="Leave"
                        glyph={ICON_LOGOUT.to_owned()}
                        danger=true
                        row_test_id={"file-tree.section.leave".to_owned()}
                        on_click={leave}
                    />
                </Show>
            }}
            on_select={chose}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Show condition={has_glyph}>
                    <Text size=TextSize::Xs color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text @sizing=ItemSize::Percent(100.0) string={label} weight=Weight::Bold />
                <Show condition={can_add}>
                    <AddChild shown={hovered} named={named.clone()} on_click={add_child} />
                </Show>
            </List>
        </ContextMenu>
    }
}

#[component]
pub(crate) fn OpenWith(editor: Editor, row: Memo<Option<Row>>) -> MenuItem {
    let block = create_memo(move || {
        row.get()
            .filter(|row| row.access.can_view())
            .and_then(|row| Some((row.id?, row.block_type)))
    });
    let editors = create_memo(move || {
        let types = editor.block_types();
        block.get().map_or_else(Vec::new, |(_, block_type)| {
            types
                .editors_for(block_type)
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
            glyph={ICON_OPEN_WITH.to_owned()}
            disabled={none}
            row_test_id={"file-tree.open-with".to_owned()}
        >
            <ForEach keys={keys}>
                {move |chosen: Uuid| {
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
                }}
            </ForEach>
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
            .and_then(|row| Some((row.id?, row.block_type, row.label)))
    });
    let shown = create_memo(move || {
        block
            .get()
            .map_or_else(Vec::new, |(_, block_type, _)| formats(block_type))
    });
    let keys = create_memo(move || {
        shown.with(|shown| shown.iter().map(|(format, _)| *format).collect::<Vec<_>>())
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    let prefix = row_test_id.clone();
    view! {
        <MenuItem
            label="Export as"
            glyph={ICON_FILE_DOWNLOAD.to_owned()}
            disabled={none}
            row_test_id={row_test_id}
        >
            <ForEach keys={keys}>
                {move |format: ExportFormat| {
                    let label = create_memo(move || {
                        shown.with(|shown| {
                            shown
                                .iter()
                                .find(|(shown, _)| *shown == format)
                                .map(|(_, label)| (*label).to_owned())
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
                                if let Some((id, block_type, name)) = block.get_untracked() {
                                    exporter.export(Export { id, block_type, name, format });
                                }
                            }}
                        />
                    }
                }}
            </ForEach>
        </MenuItem>
    }
}

#[component]
fn AddChild(
    shown: ReadSignal<bool>,
    named: String,
    on_click: block_editor_beui::beui::reactive::ClickCallback,
) -> NodeId {
    view! {
        <Frame width=ADD_WIDTH>
            <List spacing=0.0>
                <Show condition={shown}>
                    {move || view! {
                        <unstyled::Button
                            @test_id={named.clone()}
                            tab_stop=false
                            capture_presses=true
                            accessibility={add_child_accessibility()}
                            on_click={on_click}
                            content={move |button: ButtonHandle| view! {
                                <AddChildFace handle={button} />
                            }}
                        />
                    }}
                </Show>
            </List>
        </Frame>
    }
}

fn add_child_accessibility() -> AccessNode {
    let mut node = AccessNode::new(Role::Button);
    node.set_label("Add a child");
    node
}

#[component]
fn AddChildFace(handle: ButtonHandle) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let color = create_memo(move || match hovered.get() || active.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let fill = create_memo(move || match active.get() {
        true => theme.pressed.get(),
        false => Color32::TRANSPARENT,
    });
    view! {
        <Tooltip label="Add a child">
            <Frame
                width=ADD_WIDTH
                height=ADD_WIDTH
                fill={ColorRole::custom(fill)}
                border={focus_border(focused, Border::new(ColorRole::Accent).width(1.0))}
                radius={Radius::custom(3.0)}
            >
                <Text size=TextSize::Xs color={color} align=TextAlign::Center>
                    <Icon glyph={ICON_ADD.to_owned()} />
                </Text>
            </Frame>
        </Tooltip>
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
    tree: NodeRef,
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

fn row_rect(tree: NodeId, key: &RowKey) -> Option<Rect> {
    with_document(|document| {
        let node = tree_row_node::<RowKey>(document, tree, key)?;
        document.node_rect(node)
    })
}

pub(crate) fn menu_action(
    editor: Editor,
    picker: Picker,
    inspect: WriteSignal<Option<Inspection>>,
    row: Memo<Option<Row>>,
) -> impl Fn(Vec<usize>) + 'static {
    move |path: Vec<usize>| {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        match path.as_slice() {
            [1] => picker.open(
                editor,
                Some(id),
                [id].into_iter().collect::<HashSet<Uuid>>(),
            ),
            [3] => editor.host().rename_block(id),
            [4] => editor.host().share_block(id),
            [5] => {
                if let Some(container) = shown.container {
                    editor.host().unlink_block(id, container);
                }
            }
            [6] => {
                editor
                    .host()
                    .delete_block(id, shown.block_type, shown.source, shown.is_reference)
            }
            [7] => inspect.set(shown.inspection),
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Picker {
    inner: Stored<PickerInner>,
    error: block_editor_beui::beui::reactive::ReadSignal<Option<String>>,
    set_error: block_editor_beui::beui::reactive::WriteSignal<Option<String>>,
}

struct PickerInner {
    picker: std::cell::RefCell<BlockPicker>,
    target: std::cell::Cell<Option<Uuid>>,
}

impl Picker {
    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error;
        create_memo(move || error.get())
    }

    pub(crate) fn open(&self, editor: Editor, parent: Option<Uuid>, excluded: HashSet<Uuid>) {
        self.request(editor, parent, excluded, false);
    }

    pub(crate) fn open_placed(
        &self,
        editor: Editor,
        parent: Option<Uuid>,
        excluded: HashSet<Uuid>,
    ) {
        self.request(editor, parent, excluded, true);
    }

    fn request(&self, editor: Editor, parent: Option<Uuid>, excluded: HashSet<Uuid>, placed: bool) {
        self.set_error.set(None);
        let inner = self.inner.get();
        inner.target.set(parent);
        let place = parent.map_or(BlockParent::Root, BlockParent::Block);
        inner.picker.borrow_mut().open(
            &editor.host(),
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: excluded.into_iter().map(Uuid::into_bytes).collect(),
                templates: false,
                place: placed.then(|| place.encode()),
            },
        );
    }
}

fn picker(editor: Editor, tree: FileTree) -> Picker {
    let (error, set_error) = create_signal(None::<String>);
    let picker = Picker {
        inner: store(PickerInner {
            picker: std::cell::RefCell::new(BlockPicker::default()),
            target: std::cell::Cell::new(None),
        }),
        error,
        set_error,
    };
    let host = editor.host();
    editor.on_reply(move || {
        let Some(inner) = picker.inner.try_get() else {
            return;
        };
        let Some(result) = inner.picker.borrow_mut().poll(&host) else {
            return;
        };
        let target = inner.target.take();
        let picked = match result {
            Ok(picked) => picked,
            Err(error) => {
                picker.set_error.set(Some(error));
                return;
            }
        };
        tree.remember(picked.id, picked.block_type);
        if picked.placed {
            match target {
                Some(parent) => host.open_block_via(picked.id, picked.block_type, parent),
                None => host.open_block(picked.id, picked.block_type),
            }
            return;
        }
        match target {
            None => {
                if !picked.linked {
                    tree.blocks().set_parent(picked.id, BlockParent::Root);
                }
                host.open_block(picked.id, picked.block_type);
            }
            Some(parent) => {
                host.place_block(picked.id, picked.block_type, parent, picked.linked);
                host.open_block_via(picked.id, picked.block_type, parent);
            }
        }
    });
    picker
}

pub(crate) fn leavable(editor: Editor, row: Memo<Option<Row>>) -> Memo<bool> {
    let administrator = editor.host_value::<Administrator>();
    create_memo(move || !administrator.get() && row.get().is_some_and(|row| row.can_leave))
}
