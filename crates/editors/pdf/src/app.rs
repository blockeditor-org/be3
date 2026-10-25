use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use pdf_types::{PdfContent, PdfFile};

mod pages;
mod ui;

use ui::{PdfEditor, PdfPreview};

pub struct PdfApp;

#[component]
impl block_editor_beui::BeuiApp for PdfApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <PdfEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <PdfPreview editor={editor} />
            </Loaded>
        }
    }

    #[component]
    fn Creation(creation: Creation) -> NodeId {
        view! {
            <ContentFileCreation creation id_prefix="pdf" filter={filter()} import={imported} />
        }
    }
}

pub(crate) const DEFAULT_PAGE_SIZE: block_editor_beui::beui::Vec2 =
    block_editor_beui::beui::Vec2::new(612.0, 792.0);

pub(crate) fn filter() -> FileFilter {
    FileFilter::new("PDF", &["pdf"], &["application/pdf"])
}

pub(crate) fn imported(file: PickedFile) -> Result<PdfContent, String> {
    let PickedFile { name, data } = file;
    PdfFile::from_file(name.clone(), data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}
