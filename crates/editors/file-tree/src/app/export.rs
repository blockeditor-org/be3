use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::be_block::{
    AudioContent, BlockContent, ImageContent, LiveEdit, PdfContent, TextContent, TextLanguage,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Editor, FileSaver, SavedFile};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ExportFormat {
    Own,
    BlockFile,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Export {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) name: String,
    pub(crate) format: ExportFormat,
}

#[derive(Clone, Copy)]
pub(crate) struct Exporter {
    wanted: Signal<Option<Export>>,
    error: Signal<Option<String>>,
}

pub(crate) fn formats(block_type: Uuid) -> Vec<(ExportFormat, &'static str)> {
    let own = match block_type {
        kind if kind == TextContent::CONTENT_TYPE => Some("Text file"),
        kind if kind == ImageContent::CONTENT_TYPE => Some("Image file"),
        kind if kind == PdfContent::CONTENT_TYPE => Some("PDF file"),
        kind if kind == AudioContent::CONTENT_TYPE => Some("Audio file"),
        _ => None,
    };
    own.map(|label| (ExportFormat::Own, label))
        .into_iter()
        .chain([(ExportFormat::BlockFile, "Block file")])
        .collect()
}

impl Exporter {
    pub(crate) fn new(editor: Editor) -> Self {
        let wanted = create_signal(None::<Export>);
        let error = create_signal(None::<String>);
        let exporter = Self { wanted, error };
        let saver = Rc::new(RefCell::new(FileSaver::default()));
        let saving = Rc::clone(&saver);
        create_effect(move || {
            let Some(export) = wanted.get() else {
                return;
            };
            if export.format == ExportFormat::BlockFile {
                untrack(|| {
                    wanted.set(None);
                    saving.borrow_mut().export(&editor.host(), export.id);
                });
                return;
            }
            let Some(file) = file_of(editor, &export) else {
                return;
            };
            untrack(|| {
                wanted.set(None);
                saving.borrow_mut().save(&editor.host(), file);
            });
        });
        let host = editor.host();
        editor.on_reply(move || {
            if let Some(Err(failure)) = saver.borrow_mut().poll(&host) {
                error.set(Some(failure));
            }
        });
        exporter
    }

    pub(crate) fn export(&self, export: Export) {
        self.error.set(None);
        self.wanted.set(Some(export));
    }

    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error;
        create_memo(move || error.get())
    }
}

fn file_of(editor: Editor, export: &Export) -> Option<SavedFile> {
    let named = |extension: &str| file_name(&export.name, extension);
    match export.block_type {
        kind if kind == ImageContent::CONTENT_TYPE => {
            read::<ImageContent>(editor, export.id, |image| {
                let header = image.header();
                SavedFile {
                    name: source_or(&header.source_name, || {
                        named(extension_of(&header.media_type))
                    }),
                    mime_type: header.media_type.clone(),
                    data: image.data().to_vec(),
                }
            })
        }
        kind if kind == TextContent::CONTENT_TYPE => {
            read::<TextContent>(editor, export.id, |text| {
                let text = text.root();
                let (extension, mime_type) = match text.language {
                    TextLanguage::Markdown => ("md", "text/markdown"),
                    TextLanguage::PlainText => ("txt", "text/plain"),
                    TextLanguage::Rust => ("rs", "text/x-rust"),
                    TextLanguage::Zig => ("zig", "text/x-zig"),
                    TextLanguage::Toml => ("toml", "application/toml"),
                };
                SavedFile {
                    name: named(extension),
                    mime_type: mime_type.to_owned(),
                    data: text.body.bytes().to_vec(),
                }
            })
        }
        kind if kind == PdfContent::CONTENT_TYPE => {
            read::<PdfContent>(editor, export.id, |pdf| SavedFile {
                name: source_or(&pdf.header().source_name, || named("pdf")),
                mime_type: "application/pdf".to_owned(),
                data: pdf.data().to_vec(),
            })
        }
        kind if kind == AudioContent::CONTENT_TYPE => {
            read::<AudioContent>(editor, export.id, |audio| {
                let header = audio.header();
                SavedFile {
                    name: source_or(&header.source_name, || {
                        named(extension_of(&header.media_type))
                    }),
                    mime_type: header.media_type.clone(),
                    data: audio.data().to_vec(),
                }
            })
        }
        _ => None,
    }
}

fn read<C>(editor: Editor, id: Uuid, file: impl FnOnce(&C) -> SavedFile) -> Option<SavedFile>
where
    C: LiveEdit + Clone + Default,
{
    editor.content_of::<C>(id).read(file).ok()
}

fn source_or(source: &str, fallback: impl FnOnce() -> String) -> String {
    match source.trim().is_empty() {
        true => fallback(),
        false => source.trim().to_owned(),
    }
}

fn file_name(name: &str, extension: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character => character,
        })
        .collect();
    let stem = match stem.is_empty() {
        true => "Untitled".to_owned(),
        false => stem,
    };
    match extension.is_empty() {
        true => stem,
        false => format!("{stem}.{extension}"),
    }
}

fn extension_of(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        _ => "",
    }
}
