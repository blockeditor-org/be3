use block_editor_beui::be_block::PdfContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor, FileFilter, PickedFile, content_file_creation};

mod pages;
mod ui;

use ui::{PdfEditor, PdfPreview};

pub struct PdfApp;

impl block_editor_beui::BeuiApp for PdfApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PdfEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <PdfPreview editor={editor} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        content_file_creation::<PdfContent>(&creation, "pdf", filter(), imported)
    }
}

pub(crate) const DEFAULT_PAGE_SIZE: block_editor_beui::beui::Vec2 =
    block_editor_beui::beui::Vec2::new(612.0, 792.0);

pub(crate) fn filter() -> FileFilter {
    FileFilter::new("PDF", "Document.pdf", &["pdf"], &["application/pdf"])
}

pub(crate) fn imported(file: PickedFile) -> Result<PdfContent, String> {
    let PickedFile { name, data } = file;
    PdfContent::from_file(name.clone(), data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}
