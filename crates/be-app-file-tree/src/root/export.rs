use be_sdk_beui::be_sdk_ui::BlockTypes;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::{Editor, FileSave};
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
    saving: Stored<std::cell::RefCell<Option<be_sdk_beui::Task>>>,
    editor: Editor,
}

impl Exporter {
    pub(crate) fn new(editor: Editor) -> Self {
        Self {
            saving: store(std::cell::RefCell::new(None)),
            editor,
        }
    }

    pub(crate) fn export(&self, export: Export) {
        let host = self.editor.host();
        let reporter = host.clone();
        let saved = move |save: FileSave| {
            if let FileSave::Failed(failure) = save {
                reporter.report_failure(format!("The block could not be exported. {failure}"));
            }
        };
        let task = match export.format {
            ExportFormat::Own => {
                let read = host.read_plain_file(export.id);
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
            ExportFormat::BlockFile => self.editor.then(host.export_block(export.id), saved),
        };
        if let Some(earlier) = self.saving.with(|saving| saving.replace(Some(task))) {
            earlier.cancel();
        }
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
}
