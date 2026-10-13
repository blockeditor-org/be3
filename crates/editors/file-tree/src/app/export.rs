use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::be_block::{
    AudioContent, BlockContent, ImageContent, LiveEdit, PdfContent, TextContent, TextLanguage,
};
use block_editor_beui::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_effect, create_memo, create_signal, untrack,
};
use block_editor_beui::{Editor, FileSaver, SavedFile};
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub(crate) struct Export {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) name: String,
}

pub(crate) struct Exporter {
    set_wanted: WriteSignal<Option<Export>>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
}

pub(crate) fn exportable(block_type: Uuid) -> bool {
    [
        ImageContent::CONTENT_TYPE,
        TextContent::CONTENT_TYPE,
        PdfContent::CONTENT_TYPE,
        AudioContent::CONTENT_TYPE,
    ]
    .contains(&block_type)
}

impl Exporter {
    pub(crate) fn new(editor: &Editor) -> Rc<Self> {
        let (wanted, set_wanted) = create_signal(None::<Export>);
        let (error, set_error) = create_signal(None::<String>);
        let exporter = Rc::new(Self {
            set_wanted: set_wanted.clone(),
            error,
            set_error: set_error.clone(),
        });
        let saver = Rc::new(RefCell::new(FileSaver::default()));
        let saving = Rc::clone(&saver);
        let reading = editor.clone();
        create_effect(move || {
            let Some(export) = wanted.get() else {
                return;
            };
            let Some(file) = file_of(&reading, &export) else {
                return;
            };
            untrack(|| {
                set_wanted.set(None);
                saving.borrow_mut().save(reading.host(), file);
            });
        });
        let host = editor.host().clone();
        editor.on_reply(move || {
            if let Some(Err(error)) = saver.borrow_mut().poll(&host) {
                set_error.set(Some(error));
            }
        });
        exporter
    }

    pub(crate) fn export(&self, export: Export) {
        self.set_error.set(None);
        self.set_wanted.set(Some(export));
    }

    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error.clone();
        create_memo(move || error.get())
    }
}

fn file_of(editor: &Editor, export: &Export) -> Option<SavedFile> {
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

fn read<C>(editor: &Editor, id: Uuid, file: impl FnOnce(&C) -> SavedFile) -> Option<SavedFile>
where
    C: LiveEdit + Clone + Default,
{
    let content = editor.content_of::<C>(id);
    if !content.loaded().get() {
        return None;
    }
    content.read(file)
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
