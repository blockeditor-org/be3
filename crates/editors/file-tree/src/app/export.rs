use std::cell::RefCell;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{Editor, FileSaver};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ExportFormat {
    Own,
    BlockFile,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Export {
    pub(crate) id: Uuid,
    pub(crate) format: ExportFormat,
}

#[derive(Clone, Copy)]
pub(crate) struct Exporter {
    error: Signal<Option<String>>,
    saver: Stored<RefCell<FileSaver>>,
    editor: Editor,
}

impl Exporter {
    pub(crate) fn new(editor: Editor) -> Self {
        let error = create_signal(None::<String>);
        let saver = store(RefCell::new(FileSaver::default()));
        let host = editor.host();
        editor.on_reply(move || {
            if let Some(Err(failure)) = saver.try_with(|saver| saver.borrow_mut().poll(&host)).flatten() {
                error.set(Some(failure));
            }
        });
        Self {
            error,
            saver,
            editor,
        }
    }

    pub(crate) fn export(&self, export: Export) {
        self.error.set(None);
        let host = self.editor.host();
        self.saver.with(|saver| {
            let mut saver = saver.borrow_mut();
            match export.format {
                ExportFormat::Own => saver.export_plain(&host, export.id),
                ExportFormat::BlockFile => saver.export(&host, export.id),
            }
        });
    }

    pub(crate) fn formats(&self, block_type: Uuid) -> Vec<(ExportFormat, String)> {
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

    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error;
        create_memo(move || error.get())
    }
}
