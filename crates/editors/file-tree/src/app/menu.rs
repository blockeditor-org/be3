use block_editor_beui::Editor;
use block_editor_beui::be_block::{self, BlockContent, SettingsContent};
use block_editor_beui::beui::icons::{
    ICON_FILE_PLUS, ICON_INFO, ICON_LOG_OUT, ICON_MOVE_UP, ICON_PALETTE, ICON_PENCIL_LINE,
    ICON_ROTATE_CCW_CLOCK, ICON_SETTINGS, ICON_SHARE_2, ICON_TRASH, ICON_UNLINK, ICON_ZOOM_IN,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::MenuItem;
use uuid::Uuid;

use super::export::Exporter;
use super::kinds::{HomeItems, OpensIn, TagsMenu};
use super::rename::Renaming;
use super::rows::{Inspection, Row, RowKey, Tree as FileTree};
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
    pub(crate) renaming: Renaming,
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
    match (shown.id, shown.is_workspace()) {
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
            glyph=ICON_ZOOM_IN
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
    let RowTools {
        editor,
        picker,
        renaming,
        ..
    } = tools;
    let unaddable = create_memo(move || !row.get().is_some_and(|row| row.can_add));
    let edit = create_memo(move || !row.get().is_some_and(|row| row.can_edit));
    let unshareable = create_memo(move || !row.get().is_some_and(|row| row.access.can_manage()));
    let deletable = create_memo(move || !row.get().is_some_and(|row| row.can_delete));
    let my_files = be_block::my_files(editor.host().organization_id(), editor.host().account_id());
    let offers_delete = create_memo(move || !row.get().is_some_and(|row| row.id == Some(my_files)));
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
    let chose = move |path: Vec<usize>| {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        match path.as_slice() {
            [0] => zoom_in(),
            [1] => add_to_section(editor, picker, id, picker.row_anchor(&shown.key)),
            [2] => renaming.start(shown.key),
            [3] => editor.host().share_block(id),
            _ => {}
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn disabled={unzoomable} />
            <MenuItem
                label="Add"
                glyph=ICON_FILE_PLUS
                disabled={unaddable}
                row_test_id={"file-tree.section.add".to_owned()}
            />
            <MenuItem
                label="Rename"
                glyph=ICON_PENCIL_LINE
                disabled={edit}
                row_test_id={"file-tree.section.rename".to_owned()}
            />
            <MenuItem
                label="Share"
                glyph=ICON_SHARE_2
                disabled={unshareable}
                row_test_id={"file-tree.section.share".to_owned()}
            />
            <Show condition={offers_delete}>
                <MenuItem
                    label="Delete"
                    glyph=ICON_TRASH
                    disabled={deletable}
                    danger=true
                    row_test_id={"file-tree.section.delete".to_owned()}
                    on_click={delete}
                />
            </Show>
            <Appearance editor picker row />
            <BlockSettings editor row row_test_id={"file-tree.section.settings".to_owned()} />
            <Show condition={leavable}>
                <MenuItem
                    label="Leave"
                    glyph=ICON_LOG_OUT
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
        renaming,
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
    let unshareable = create_memo(move || !row.get().is_some_and(|row| row.access.can_manage()));
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
    let uninspectable = create_memo(move || !row.get().is_some_and(|row| row.inspection.is_some()));
    let taggable = create_memo(move || row.get().is_some_and(|row| row.can_edit && !row.deleted));
    let choosable = create_memo(move || {
        row.get().is_some_and(|row| {
            row.can_edit && editor.block_types().editors_for(row.block_type).len() > 1
        })
    });
    let homeable = create_memo(move || {
        row.get().is_some_and(|row| {
            !row.deleted
                && row.id.is_some_and(|id| {
                    editor.workspace_of(id).is_some_and(|workspace| {
                        workspace != id && editor.blocks().access(workspace).can_manage()
                    })
                })
        })
    });
    let rows = tree.rows();
    let destinations = create_memo(move || {
        let here = row.get().and_then(|row| row.container);
        rows.with(|rows| {
            rows.iter()
                .filter(|other| other.is_workspace())
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
            [2] => picker.open(
                editor,
                id,
                [id].into_iter().collect(),
                picker.row_anchor(&shown.key),
            ),
            [4] => renaming.start(shown.key),
            [5] => editor.host().share_block(id),
            [6] => {
                if let Some(container) = shown.container {
                    editor.host().unlink_block(id, container);
                }
            }
            [7] => editor
                .host()
                .delete_block(id, shown.block_type, shown.source),
            [8] => inspect.set(shown.inspection),
            _ => {}
        }
    };
    TreeMenu {
        items: view! {
            <ZoomIn disabled={unzoomable} />
            <OpenWith editor={editor} row={row} />
            <MenuItem label="Add" glyph=ICON_FILE_PLUS disabled={add} />
            <MenuItem
                label="Move to"
                glyph=ICON_MOVE_UP
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
            </MenuItem>
            <MenuItem label="Rename" glyph=ICON_PENCIL_LINE disabled={edit} />
            <MenuItem label="Share" glyph=ICON_SHARE_2 disabled={unshareable} />
            <MenuItem label="Unlink" glyph=ICON_UNLINK disabled={unlinkable} />
            <MenuItem label={delete_label} glyph=ICON_TRASH disabled={deletable} danger=true />
            <MenuItem label="Inspect" glyph=ICON_INFO disabled={uninspectable} />
            <ExportAs exporter row row_test_id={"file-tree.export".to_owned()} />
            <Show condition={restorable}>
                <MenuItem
                    label="Restore"
                    glyph=ICON_ROTATE_CCW_CLOCK
                    row_test_id={"file-tree.restore".to_owned()}
                    on_click={restore}
                />
            </Show>
            <Appearance editor picker row />
            <BlockSettings editor row row_test_id={"file-tree.settings".to_owned()} />
            <Show condition={taggable}>
                <TagsMenu editor row />
            </Show>
            <Show condition={choosable}>
                <OpensIn editor row />
            </Show>
            <Show condition={homeable}>
                <HomeItems editor row />
            </Show>
            <Show condition={leavable}>
                <MenuItem
                    label="Leave"
                    glyph=ICON_LOG_OUT
                    danger=true
                    row_test_id={"file-tree.leave".to_owned()}
                    on_click={leave}
                />
            </Show>
        },
        on_select: Callback::new(chose),
    }
}

#[component]
fn Appearance(editor: Editor, picker: Picker, row: Memo<Option<Row>>) -> MenuItem {
    let disabled = create_memo(move || !row.get().is_some_and(|row| row.can_edit));
    let open = move || {
        if let Some(shown) = row.get_untracked().filter(|row| row.can_edit)
            && let Some(id) = shown.id
        {
            editor
                .host()
                .rename_block(id, picker.row_anchor(&shown.key));
        }
    };
    view! {
        <MenuItem
            label="Icon and color…"
            glyph=ICON_PALETTE
            disabled
            row_test_id="file-tree.appearance"
            on_click={open}
        />
    }
}

#[component]
fn BlockSettings(editor: Editor, row: Memo<Option<Row>>, row_test_id: String) -> MenuItem {
    let shown = create_memo(move || row.get().and_then(|row| row.id));
    let has = create_memo(move || shown.get().and_then(|id| editor.settings_block(id)));
    let disabled = create_memo(move || {
        has.get().is_none() && !row.get().is_some_and(|row| row.access.can_manage())
    });
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
        <MenuItem label="Settings" glyph=ICON_SETTINGS disabled row_test_id on_click={open} />
    }
}
