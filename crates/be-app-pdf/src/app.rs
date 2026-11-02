use be_sdk_beui::Loaded;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use be_type_pdf::{PdfContent, PdfFile};

mod pages;
mod ui;

use ui::{PdfEditor, PdfPreview};

pub struct PdfApp;

#[component]
impl be_sdk_beui::BeuiApp for PdfApp {
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
            <ContentFileCreation
                creation
                title="New PDF"
                id_prefix="pdf"
                filter={filter()}
                import={imported}
            />
        }
    }
}

pub(crate) const DEFAULT_PAGE_SIZE: be_sdk_beui::beui::Vec2 =
    be_sdk_beui::beui::Vec2::new(612.0, 792.0);

pub(crate) fn filter() -> FileFilter {
    FileFilter::new("PDF", &["pdf"], &["application/pdf"])
}

pub(crate) fn imported(file: PickedFile) -> Result<PdfContent, String> {
    let PickedFile { name, data } = file;
    PdfFile::from_file(name.clone(), data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}
