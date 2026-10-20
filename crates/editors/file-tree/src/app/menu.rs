use block_editor_beui::BlockParent;
use block_editor_beui::Editor;
use block_editor_beui::beui::icons::{
    ICON_DELETE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_INFO, ICON_LINK_OFF, ICON_LOGOUT,
    ICON_MOVE_UP, ICON_NOTE_ADD, ICON_RESTORE, ICON_SHARE, ICON_ZOOM_IN,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::MenuItem;
use uuid::Uuid;

use super::export::Exporter;
use super::rows::{Inspection, Row, RowKey, Tree as FileTree, is_workspace};
use super::ui::{ExportAs, OpenWith, Picker, add_to_section, leavable};
use super::zoom::{Zoom, zooms};

#[derive(Clone, Copy)]
pub(crate) struct RowTools {
    pub(crate) editor: Editor,
    pub(crate) tree: FileTree,
    pub(crate) picker: Picker,
    pub(crate) exporter: Exporter,
    pub(crate) inspect: Signal<Option<Inspection>>,
    pub(crate) zoom: Zoom,
}

#[component]
pub(crate) fn RowMenu(tools: RowTools, key: RowKey) -> Option<TreeMenu> {
    let row = tools.tree.row(key.clone());
    let shown = row.get_untracked()?;
    let zoomed = tools.zoom.zoomed();
    let entered = key.clone();
    let unzoomable = create_memo(move || {
        zoomed.get().as_ref() == Some(&entered)
            || !row.get().is_some_and(|row| zooms(&row) || row.expandable)
    });
    let zoom = tools.zoom;
    let zoom_in = move || zoom.enter(key.clone());
    match (shown.id, shown.section && is_workspace(shown.block_type)) {
        (None, _) if !shown.section => None,
        (None, _) => Some(TreeMenu {
            items: Children::from([into_segment(view! {
                <ZoomIn disabled={unzoomable} />
            })]),
            on_select: Callback::new(move |path: Vec<usize>| {
                if path == [0] {
                    zoom_in();
                }
            }),
        }),
        (Some(_), true) => Some(view! {
            <SectionMenu tools row unzoomable zoom_in />
        }),
        (Some(_), false) => Some(view! {
            <BlockMenu tools row unzoomable zoom_in />
        }),
    }
}

#[component]
fn ZoomIn(disabled: Memo<bool>) -> MenuItem {
    view! {
        <MenuItem
            label="Zoom in"
            glyph={ICON_ZOOM_IN.to_owned()}
            disabled={disabled}
            row_test_id={"file-tree.zoom-in".to_owned()}
        />
    }
}

#[component]
fn SectionMenu<F: Fn() + 'static>(
    tools: RowTools,
    row: Memo<Option<Row>>,
    unzoomable: Memo<bool>,
    zoom_in: F,
) -> TreeMenu {
    let RowTools { editor, picker, .. } = tools;
    let unaddable = create_memo(move || !row.get().is_some_and(|row| row.can_add));
    let edit = create_memo(move || !row.get().is_some_and(|row| row.can_edit));
    let deletable = create_memo(move || !row.get().is_some_and(|row| row.can_delete));
    let leavable = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let chose = move |path: Vec<usize>| {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        match path.as_slice() {
            [0] => zoom_in(),
            [1] => add_to_section(editor, picker, id),
            [2] => editor.host().rename_block(id),
            [3] => editor.host().share_block(id),
            [4] => editor
                .host()
                .delete_block(id, shown.block_type, shown.source, false),
            _ => {}
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn disabled={unzoomable} />
            <MenuItem
                label="Add"
                glyph={ICON_NOTE_ADD.to_owned()}
                disabled={unaddable}
                row_test_id={"file-tree.section.add".to_owned()}
            />
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
        },
        on_select: Callback::new(chose),
    }
}

#[component]
fn BlockMenu<F: Fn() + 'static>(
    tools: RowTools,
    row: Memo<Option<Row>>,
    unzoomable: Memo<bool>,
    zoom_in: F,
) -> TreeMenu {
    let RowTools {
        editor,
        tree,
        picker,
        exporter,
        inspect,
        ..
    } = tools;
    let leavable = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let restorable = create_memo(move || row.get().is_some_and(|row| row.deleted && row.can_edit));
    let restore = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().restore_block(id);
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
                .is_some_and(|row| row.parent == Some(BlockParent::Detached))
    });
    let uninspectable = create_memo(move || !row.get().is_some_and(|row| row.inspection.is_some()));
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
    let chose = move |path: Vec<usize>| {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        match path.as_slice() {
            [0] => zoom_in(),
            [2] => picker.open(editor, Some(id), [id].into_iter().collect()),
            [4] => editor.host().rename_block(id),
            [5] => editor.host().share_block(id),
            [6] => {
                if let Some(container) = shown.container {
                    editor.host().unlink_block(id, container);
                }
            }
            [7] => {
                editor
                    .host()
                    .delete_block(id, shown.block_type, shown.source, shown.is_reference)
            }
            [8] => inspect.set(shown.inspection),
            _ => {}
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn disabled={unzoomable} />
            <OpenWith editor={editor} row={row} />
            <MenuItem label="Add" glyph={ICON_NOTE_ADD.to_owned()} disabled={add} />
            <MenuItem
                label="Move to"
                glyph={ICON_MOVE_UP.to_owned()}
                disabled={rooted}
                row_test_id={"file-tree.move-to".to_owned()}
            >
                <ForEach keys={destination_keys}>|target: Uuid| {
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
                }</ForEach>
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
            <Show condition={restorable}>
                <MenuItem
                    label="Restore"
                    glyph={ICON_RESTORE.to_owned()}
                    row_test_id={"file-tree.restore".to_owned()}
                    on_click={restore}
                />
            </Show>
            <Show condition={leavable}>
                <MenuItem
                    label="Leave"
                    glyph={ICON_LOGOUT.to_owned()}
                    danger=true
                    row_test_id={"file-tree.leave".to_owned()}
                    on_click={leave}
                />
            </Show>
        },
        on_select: Callback::new(chose),
    }
}
