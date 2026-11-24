use be_sdk_beui::be_block::{self, SettingsContent};
use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::beui::ClipItem;
use be_sdk_beui::beui::clipboard::Blocks;
use be_sdk_beui::beui::icons::{
    ICON_CLIPBOARD_PASTE, ICON_COPY, ICON_FILE_PLUS, ICON_INFO, ICON_LOG_OUT, ICON_PALETTE,
    ICON_PENCIL_LINE, ICON_ROTATE_CCW_CLOCK, ICON_SCISSORS, ICON_SETTINGS, ICON_SHARE_2,
    ICON_TRASH, ICON_UNLINK, ICON_ZOOM_IN,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::block_menu::{DeleteBlock, ExportAs, Exporter, MenuBlock, MoveTo};
use be_sdk_beui::{BlockParent, Editor};
use uuid::Uuid;

use super::kinds::{HomeItems, OpensIn, TagsMenu};
use super::rename::Renaming;
use super::rows::{Inspection, Row, RowKey, Tree as FileTree};
use super::ui::{OpenWith, Picker, add_to_section, leavable};
use super::zoom::Zoom;

#[derive(Clone, Copy)]
pub(crate) struct RowTools {
    pub(crate) editor: Editor,
    pub(crate) tree: FileTree,
    pub(crate) picker: Picker,
    pub(crate) exporter: Exporter,
    pub(crate) inspect: Signal<Option<Inspection>>,
    pub(crate) inspected_from: NodeRef,
    pub(crate) zoom: Zoom,
    pub(crate) renaming: Renaming,
    pub(crate) tree_api: ComponentRef<TreeApi<RowKey>>,
}

#[component]
pub(crate) fn RowMenu(tools: RowTools, key: RowKey) -> Option<TreeMenu> {
    let row = tools.tree.row(key.clone());
    let shown = row.get_untracked()?;
    let zoom = tools.zoom;
    let unfiled = shown.key == RowKey::Unfiled;
    let zoom_in = move || zoom.enter(key.clone());
    if unfiled {
        return Some(view! {
            <UnfiledMenu tools zoom_in />
        });
    }
    match (shown.id, shown.is_workspace()) {
        (None, _) if !shown.section => None,
        (None, _) => Some(TreeMenu {
            items: Children::from([into_segment(view! {
                <ZoomIn on_click={zoom_in} />
            })]),
        }),
        (Some(_), true) => Some(view! {
            <SectionMenu tools row zoom_in />
        }),
        (Some(_), false) => Some(view! {
            <BlockMenu tools row zoom_in />
        }),
    }
}

#[component]
fn ZoomIn(on_click: Callback<()>) -> MenuItem {
    view! {
        <MenuItem
            label="Zoom in"
            glyph=ICON_ZOOM_IN
            @test_id={"file-tree.zoom-in".to_owned()}
            on_click
        />
    }
}

/// Unfiled's own menu: besides zooming in, the one thing to do with the whole section is to
/// move what nothing uses to Recently Deleted. Nothing there is deleted automatically.
#[component]
fn UnfiledMenu<F: Fn() + 'static>(tools: RowTools, zoom_in: F) -> TreeMenu {
    let RowTools { editor, tree, .. } = tools;
    let rows = tree.rows();
    let unused = create_memo(move || {
        rows.with(|rows| {
            rows.iter()
                .filter(|row| row.unused && row.can_delete)
                .filter_map(|row| Some((row.id?, row.block_type)))
                .collect::<Vec<_>>()
        })
    });
    let none = move || unused.with(Vec::is_empty);
    let move_unused = move || {
        for (id, block_type) in unused.get_untracked() {
            editor
                .host()
                .delete_block(id, block_type, BlockParent::Unfiled);
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn on_click={zoom_in} />
            <MenuItem
                label="Move unused to Recently Deleted"
                glyph=ICON_TRASH
                disabled={none}
                @test_id={"file-tree.unfiled.move-unused".to_owned()}
                on_click={move_unused}
            />
        },
    }
}

#[component]
fn SectionMenu<F: Fn() + 'static>(tools: RowTools, row: Memo<Option<Row>>, zoom_in: F) -> TreeMenu {
    let RowTools {
        editor,
        picker,
        renaming,
        ..
    } = tools;
    let unaddable = move || !row.with(|row| row.as_ref().is_some_and(|row| row.can_add));
    let edit = move || !row.with(|row| row.as_ref().is_some_and(|row| row.can_edit));
    let unshareable =
        move || !row.with(|row| row.as_ref().is_some_and(|row| row.access.can_manage()));
    let deletable = move || !row.with(|row| row.as_ref().is_some_and(|row| row.can_delete));
    let my_files = be_block::my_files(editor.host().organization_id(), editor.host().account_id());
    let offers_delete =
        move || !row.with(|row| row.as_ref().is_some_and(|row| row.id == Some(my_files)));
    let delete = move || {
        if let Some(shown) = row.get_untracked()
            && let Some(id) = shown.id
        {
            editor
                .host()
                .delete_block(id, shown.block_type, shown.source);
        }
    };
    let leavable = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let shown = move || row.get_untracked().and_then(|row| Some((row.id?, row)));
    let add = move || {
        if let Some((id, shown)) = shown() {
            add_to_section(editor, picker, id, picker.row_anchor(&shown.key));
        }
    };
    let rename = move || {
        if let Some((_, shown)) = shown() {
            renaming.start(shown.key);
        }
    };
    let share = move || {
        if let Some((id, _)) = shown() {
            editor.host().share_block(id);
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn on_click={zoom_in} />
            <MenuItem
                label="Add"
                glyph=ICON_FILE_PLUS
                disabled={unaddable}
                @test_id={"file-tree.section.add".to_owned()}
                on_click={add}
            />
            <MenuItem
                label="Rename"
                glyph=ICON_PENCIL_LINE
                disabled={edit}
                @test_id={"file-tree.section.rename".to_owned()}
                on_click={rename}
            />
            <MenuItem
                label="Share"
                glyph=ICON_SHARE_2
                disabled={unshareable}
                @test_id={"file-tree.section.share".to_owned()}
                on_click={share}
            />
            <Show value={offers_delete}>
                <MenuItem
                    label="Delete"
                    glyph=ICON_TRASH
                    disabled={deletable}
                    danger=true
                    @test_id={"file-tree.section.delete".to_owned()}
                    on_click={delete}
                />
            </Show>
            <Appearance editor picker row />
            <BlockSettings editor row row_test_id={"file-tree.section.settings".to_owned()} />
            <Show value={leavable}>
                <MenuItem
                    label="Leave"
                    glyph=ICON_LOG_OUT
                    danger=true
                    @test_id={"file-tree.section.leave".to_owned()}
                    on_click={leave}
                />
            </Show>
        },
    }
}

#[component]
fn BlockMenu<F: Fn() + 'static>(tools: RowTools, row: Memo<Option<Row>>, zoom_in: F) -> TreeMenu {
    let RowTools {
        editor,
        tree,
        picker,
        exporter,
        inspect,
        inspected_from,
        renaming,
        tree_api,
        ..
    } = tools;
    let leavable = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let restorable =
        move || row.with(|row| row.as_ref().is_some_and(|row| row.deleted && row.can_edit));
    let restore = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().restore_block(id);
        }
    };
    let add = create_memo(move || !row.with(|row| row.as_ref().is_some_and(|row| row.can_add)));
    let edit = create_memo(move || !row.with(|row| row.as_ref().is_some_and(|row| row.can_edit)));
    let unshareable =
        move || !row.with(|row| row.as_ref().is_some_and(|row| row.access.can_manage()));
    let unlinkable = move || {
        !row.get()
            .is_some_and(|row| row.is_reference && row.unlink.is_ok())
    };
    let rooted = create_memo(move || {
        edit.get() || !row.with(|row| row.as_ref().is_some_and(|row| row.can_delete))
    });
    let uninspectable =
        move || !row.with(|row| row.as_ref().is_some_and(|row| row.inspection.is_some()));
    let taggable =
        move || row.with(|row| row.as_ref().is_some_and(|row| row.can_edit && !row.deleted));
    let choosable = move || {
        row.with(|row| {
            row.as_ref().is_some_and(|row| {
                row.can_edit
                    && editor
                        .block_types()
                        .editors_for(row.block_type, row.file_name.as_deref())
                        .len()
                        > 1
            })
        })
    };
    let homeable = move || {
        row.with(|row| {
            row.as_ref().is_some_and(|row| {
                !row.deleted
                    && row.id.is_some_and(|id| {
                        editor.workspace_of(id).is_some_and(|workspace| {
                            workspace != id && editor.blocks().access(workspace).can_manage()
                        })
                    })
            })
        })
    };
    let rows = tree.rows();
    let destinations = create_memo(move || {
        rows.with(|rows| {
            rows.iter()
                .filter(|other| other.is_workspace())
                .filter_map(|other| Some((other.id?, other.label.clone())))
                .collect::<Vec<_>>()
        })
    });
    let block = menu_block(row);
    let shown = move || row.get_untracked().and_then(|row| Some((row.id?, row)));
    let add_into = move || {
        if let Some((id, shown)) = shown() {
            picker.open(
                editor,
                id,
                [id].into_iter().collect(),
                picker.row_anchor(&shown.key),
            );
        }
    };
    let rename = move || {
        if let Some((_, shown)) = shown() {
            renaming.start(shown.key);
        }
    };
    let copy_row = move || {
        if let Some((id, shown)) = shown() {
            editor.copy_blocks(&[(id, shown.block_type, shown.label.clone())]);
        }
    };
    let paste = move || {
        // The paste reaches the focus, so the row the menu was opened on takes it.
        if let Some((_, shown)) = shown().filter(|(_, shown)| shown.can_add) {
            if let Some(api) = tree_api.try_get() {
                api.focus(shown.key);
            }
            be_sdk_beui::beui::reactive::request_paste();
        }
    };
    let cut = move || {
        if let Some((id, shown)) = shown() {
            editor.copy_blocks(&[(id, shown.block_type, shown.label.clone())]);
            if let Some(container) = shown.container {
                editor.host().cut_blocks(&[id], container);
            }
        }
    };
    let share = move || {
        if let Some((id, _)) = shown() {
            editor.host().share_block(id);
        }
    };
    let unlink = move || {
        if let Some((id, shown)) = shown()
            && let Some(container) = shown.container
        {
            editor.host().unlink_block(id, container);
        }
    };
    let inspecting = move || {
        if let Some((_, shown)) = shown() {
            if let Some(row) = focused_node() {
                inspected_from.fill(row);
            }
            inspect.set(shown.inspection);
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn on_click={zoom_in} />
            <OpenWith editor={editor} row={row} />
            <MenuItem label="Add" glyph=ICON_FILE_PLUS disabled={add} on_click={add_into} />
            <MoveTo editor block destinations named={"file-tree.move-to".to_owned()} />
            <MenuItem label="Rename" glyph=ICON_PENCIL_LINE disabled={edit} on_click={rename} />
            <MenuItem
                label="Copy"
                glyph=ICON_COPY
                @test_id={"file-tree.copy".to_owned()}
                on_click={copy_row}
            />
            <MenuItem
                label="Cut"
                glyph=ICON_SCISSORS
                disabled={rooted}
                @test_id={"file-tree.cut".to_owned()}
                on_click={cut}
            />
            <MenuItem
                label="Paste"
                glyph=ICON_CLIPBOARD_PASTE
                disabled={add}
                @test_id={"file-tree.paste".to_owned()}
                on_click={paste}
            />
            <MenuItem label="Share" glyph=ICON_SHARE_2 disabled={unshareable} on_click={share} />
            <MenuItem label="Unlink" glyph=ICON_UNLINK disabled={unlinkable} on_click={unlink} />
            <DeleteBlock editor block />
            <MenuItem
                label="Inspect"
                glyph=ICON_INFO
                disabled={uninspectable}
                on_click={inspecting}
            />
            <ExportAs exporter block named={"file-tree.export".to_owned()} />
            <Show value={restorable}>
                <MenuItem
                    label="Restore"
                    glyph=ICON_ROTATE_CCW_CLOCK
                    @test_id={"file-tree.restore".to_owned()}
                    on_click={restore}
                />
            </Show>
            <Appearance editor picker row />
            <BlockSettings editor row row_test_id={"file-tree.settings".to_owned()} />
            <Show value={taggable}>
                <TagsMenu editor row />
            </Show>
            <Show value={choosable}>
                <OpensIn editor row />
            </Show>
            <Show value={homeable}>
                <HomeItems editor row />
            </Show>
            <Show value={leavable}>
                <MenuItem
                    label="Leave"
                    glyph=ICON_LOG_OUT
                    danger=true
                    @test_id={"file-tree.leave".to_owned()}
                    on_click={leave}
                />
            </Show>
        },
    }
}

#[component]
fn Appearance(editor: Editor, picker: Picker, row: Memo<Option<Row>>) -> MenuItem {
    let disabled = move || !row.with(|row| row.as_ref().is_some_and(|row| row.can_edit));
    let open = move || {
        if let Some(shown) = row.get_untracked().filter(|row| row.can_edit)
            && let Some(id) = shown.id
        {
            editor.rename_block(id, picker.row_anchor(&shown.key));
        }
    };
    view! {
        <MenuItem
            label="Icon and color…"
            glyph=ICON_PALETTE
            disabled
            @test_id="file-tree.appearance"
            on_click={open}
        />
    }
}

#[component]
fn BlockSettings(editor: Editor, row: Memo<Option<Row>>, row_test_id: String) -> MenuItem {
    let shown = create_memo(move || row.get().and_then(|row| row.id));
    let has = create_memo(move || shown.get().and_then(|id| editor.settings_block(id)));
    let disabled = move || {
        has.with(Option::is_none)
            && !row.with(|row| row.as_ref().is_some_and(|row| row.access.can_manage()))
    };
    let open = move || {
        if let Some(id) = shown.get_untracked()
            && let Some(settings) = editor.ensure_settings_block(id)
        {
            editor
                .host()
                .open_block(settings, SettingsContent::CONTENT_TYPE);
        }
    };
    view! {
        <MenuItem
            label="Settings"
            glyph=ICON_SETTINGS
            disabled
            @test_id={row_test_id}
            on_click={open}
        />
    }
}

/// Pastes a `be3/blocks` clip into the folder `row` shows. The host
/// (`paste-blocks`) moves a block cut from its parent there and answers with
/// the blocks this editor may view. A link whose cut the paste used up moves
/// out of the block the host says it was cut from; every other block answered
/// is linked into the folder here, as the add picker links a block.
pub(crate) fn paste_into(editor: Editor, row: Option<Row>, pasted: ClipItem) {
    let Some(folder) = row.filter(|row| row.can_add).and_then(|row| row.id) else {
        return;
    };
    let Some(blocks) = pasted.decode::<Blocks>() else {
        return;
    };
    let ids: Vec<Uuid> = blocks.blocks.iter().map(|block| block.id).collect();
    let host = editor.host();
    let asked = host.paste_blocks(&ids, BlockParent::Block(folder));
    let _ = editor.then(asked, move |pasted| {
        for pasted in pasted {
            let (block, block_type) = (pasted.block, pasted.block_type);
            match pasted.cut_from {
                _ if pasted.moved => {}
                Some(source) if source == folder => {}
                Some(source) => {
                    host.move_block(block, block_type, BlockParent::Block(source), folder, true)
                }
                None => host.place_block(block, block_type, folder, true),
            }
        }
    });
}

/// A block row as the shared menu items act on it (be_sdk_beui's `block_menu`).
fn menu_block(row: Memo<Option<Row>>) -> Memo<Option<MenuBlock>> {
    create_memo(move || {
        row.get().and_then(|row| {
            Some(MenuBlock {
                id: row.id?,
                block_type: row.block_type,
                source: row.source,
                is_reference: row.is_reference,
                access: row.access,
                can_delete: row.can_delete,
            })
        })
    })
}
