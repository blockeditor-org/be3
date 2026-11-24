//! The items of a block row's menu that every editor listing blocks offers the same way (the
//! files pane, a folder): move it to a workspace, delete it, export it, and rename it, refusing
//! a name too long with a toast that says why. Each editor puts them in its own menu, beside its
//! own items, for a `MenuBlock` it describes its row as.

use std::cell::RefCell;

use be_sdk::be_sdk_ui::BlockTypes;
use beui::icons::{ICON_DOWNLOAD, ICON_MOVE_UP, ICON_TRASH};
use beui::reactive::*;
use beui::styled::*;
use uuid::Uuid;

use crate::{AccessLevel, BlockParent, Blocks, Editor, FileSave, Task};

/// A block as a row's menu acts on it.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuBlock {
    pub id: Uuid,
    pub block_type: Uuid,
    /// Where the row lists it: the block it is listed in, or the top of the files pane.
    pub source: BlockParent,
    /// It is listed in a block that only links to it: it lives elsewhere.
    pub is_reference: bool,
    pub access: AccessLevel,
    /// It may be taken out of where it is listed (`can_move_out_of`).
    pub can_delete: bool,
}

/// Whether `child`, listed in `source`, may be taken out of it: deleted, moved or cut. A block
/// listed in another needs that block's type to let children go and edit access to it, and a
/// block of its own (not a link) edit access to itself too. `block_type` answers the type of a
/// block it may be listed in.
pub fn can_move_out_of(
    client: &Blocks,
    types: &dyn BlockTypes,
    block_type: impl Fn(Uuid) -> Option<Uuid>,
    source: BlockParent,
    child: Uuid,
    is_reference: bool,
) -> bool {
    let can_delete = match source {
        BlockParent::Root | BlockParent::Detached | BlockParent::Unfiled => true,
        BlockParent::Block(id) => {
            block_type(id).is_some_and(|block_type| types.child_operations(block_type).delete)
                && client.access(id).can_edit()
        }
    };
    can_delete && (is_reference || client.access(child).can_edit())
}

/// The toast that says why a rename was refused, one at a time.
pub const RENAME_REFUSED: u64 = 0x6669_6c65_7472_6565;

/// Names block `id` `name`. Refused (a name too long), the name stays as it was, and the editor
/// says why.
pub fn rename_block(editor: Editor, id: Uuid, name: &str) {
    if let Err(reason) = editor.blocks().set_name(id, Some(name.to_owned())) {
        editor.notify(Toast {
            id: RENAME_REFUSED,
            title: "Not renamed".to_owned(),
            message: reason,
            danger: true,
            ..Toast::default()
        });
    }
}

/// What a block is exported as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// The file of its own kind (a text block's text file), for a type that has one.
    Own,
    /// A block file, which any host imports whole.
    BlockFile,
}

/// Saves blocks as files, one at a time: an export started cancels the one before.
#[derive(Clone, Copy)]
pub struct Exporter {
    saving: Stored<RefCell<Option<Task>>>,
    editor: Editor,
}

impl Exporter {
    pub fn new(editor: Editor) -> Self {
        Self {
            saving: store(RefCell::new(None)),
            editor,
        }
    }

    /// Saves block `id` as `format`, reporting a failure.
    pub fn export(&self, id: Uuid, format: ExportFormat) {
        let host = self.editor.host();
        let reporter = host.clone();
        let saved = move |save: FileSave| {
            if let FileSave::Failed(failure) = save {
                reporter.report_failure(format!("The block could not be exported. {failure}"));
            }
        };
        let task = match format {
            ExportFormat::Own => {
                let read = host.read_plain_file(id);
                self.editor.then(
                    async move {
                        match read.await? {
                            Ok(file) => host.save_file(file).await,
                            Err(error) => Some(FileSave::Failed(error.to_string())),
                        }
                    },
                    saved,
                )
            }
            ExportFormat::BlockFile => self.editor.then(host.export_block(id), saved),
        };
        if let Some(earlier) = self.saving.with(|saving| saving.replace(Some(task))) {
            earlier.cancel();
        }
    }

    /// What a block of `block_type` exports as, each with its label.
    pub fn formats(&self, block_type: Uuid) -> Vec<(ExportFormat, String)> {
        let types = self.editor.block_types();
        let own = types.plain_file(block_type).then(|| {
            let name = types.display_name(block_type).unwrap_or_default();
            match name.to_lowercase().ends_with("file") {
                true => name.to_owned(),
                false => format!("{name} file"),
            }
        });
        own.map(|label| (ExportFormat::Own, label))
            .into_iter()
            .chain([(ExportFormat::BlockFile, "Block file".to_owned())])
            .collect()
    }
}

/// "Export as", named `named` for tests, with an item for each format `block` exports as,
/// `{named}.own` and `{named}.block`.
#[component]
pub fn ExportAs(exporter: Exporter, block: Memo<Option<MenuBlock>>, named: String) -> MenuItem {
    let block = create_memo(move || {
        block
            .get()
            .filter(|block| block.access.can_view())
            .map(|block| (block.id, block.block_type))
    });
    let shown = create_memo(move || {
        block
            .get()
            .map_or_else(Vec::new, |(_, block_type)| exporter.formats(block_type))
    });
    let keys = create_memo(move || {
        shown.with(|shown| shown.iter().map(|(format, _)| *format).collect::<Vec<_>>())
    });
    let prefix = named.clone();
    view! {
        <MenuItem
            label="Export as"
            glyph=ICON_DOWNLOAD
            disabled={move || keys.with(Vec::is_empty)}
            @test_id={named}
        >
            <ForEach value={keys}>|format: ExportFormat| {
                let label = move || {
                    shown.with(|shown| {
                        shown
                            .iter()
                            .find(|(shown, _)| *shown == format)
                            .map(|(_, label)| label.clone())
                            .unwrap_or_default()
                    })
                };
                let item = match format {
                    ExportFormat::Own => format!("{prefix}.own"),
                    ExportFormat::BlockFile => format!("{prefix}.block"),
                };
                view! {
                    <MenuItem
                        label={label}
                        @test_id={item}
                        on_click={move || {
                            if let Some((id, _)) = block.get_untracked() {
                                exporter.export(id, format);
                            }
                        }}
                    />
                }
            }</ForEach>
        </MenuItem>
    }
}

/// "Move to", with an item for each of `destinations` (a workspace's id and name) that `block`
/// is not already in; `named` names it for tests, and `{named}.{id}` each. A block that may not be taken out of where it
/// is, or not edited, stays.
#[component]
pub fn MoveTo(
    editor: Editor,
    block: Memo<Option<MenuBlock>>,
    destinations: Memo<Vec<(Uuid, String)>>,
    named: String,
) -> MenuItem {
    let shown = create_memo(move || {
        let here = block.with(|block| block.as_ref().and_then(|block| block.source.block()));
        destinations.with(|shown| {
            shown
                .iter()
                .filter(|(id, _)| Some(*id) != here)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let keys =
        create_memo(move || shown.with(|shown| shown.iter().map(|(id, _)| *id).collect::<Vec<_>>()));
    let rooted = move || {
        !block.with(|block| {
            block
                .as_ref()
                .is_some_and(|block| block.access.can_edit() && block.can_delete)
        })
    };
    let prefix = named.clone();
    view! {
        <MenuItem label="Move to" glyph=ICON_MOVE_UP disabled={rooted} @test_id={named}>
            <ForEach value={keys}>|target: Uuid| {
                let label = move || {
                    shown.with(|shown| {
                        shown
                            .iter()
                            .find(|(id, _)| *id == target)
                            .map(|(_, name)| name.clone())
                            .unwrap_or_default()
                    })
                };
                view! {
                    <MenuItem
                        label={label}
                        @test_id={format!("{prefix}.{target}")}
                        on_click={move || {
                            if let Some(block) = block.get_untracked() {
                                editor.host().move_block(
                                    block.id,
                                    block.block_type,
                                    block.source,
                                    target,
                                    block.is_reference,
                                );
                            }
                        }}
                    />
                }
            }</ForEach>
        </MenuItem>
    }
}

/// "Delete", or "Remove link" for a link: takes `block` out of where it is listed, into Recently
/// Deleted unless it is only a link.
#[component]
pub fn DeleteBlock(editor: Editor, block: Memo<Option<MenuBlock>>) -> MenuItem {
    let label = move || match block.with(|block| block.as_ref().is_some_and(|block| block.is_reference)) {
        true => "Remove link".to_owned(),
        false => "Delete".to_owned(),
    };
    let disabled = move || !block.with(|block| block.as_ref().is_some_and(|block| block.can_delete));
    let delete = move || {
        if let Some(block) = block.get_untracked() {
            editor
                .host()
                .delete_block(block.id, block.block_type, block.source);
        }
    };
    view! {
        <MenuItem label={label} glyph=ICON_TRASH disabled={disabled} danger=true on_click={delete} />
    }
}
