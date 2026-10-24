use uuid::Uuid;

use crate::{
    AudioContent, BlockContent, ContentError, Entries, FileContent, FolderContent, ImageContent,
    PdfContent, TextBlock, TextContent, TextLanguage, WorkspaceContent, derived_metadata,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    Directory,
    File,
    None,
}

pub fn form(block_type: Uuid) -> Form {
    match block_type {
        kind if kind == FolderContent::CONTENT_TYPE || kind == WorkspaceContent::CONTENT_TYPE => {
            Form::Directory
        }
        kind if kind == TextContent::CONTENT_TYPE
            || kind == ImageContent::CONTENT_TYPE
            || kind == PdfContent::CONTENT_TYPE
            || kind == AudioContent::CONTENT_TYPE
            || kind == FileContent::CONTENT_TYPE =>
        {
            Form::File
        }
        _ => Form::None,
    }
}

pub fn file_bytes(block_type: Uuid, content: &[u8]) -> Result<Vec<u8>, ContentError> {
    Ok(match block_type {
        kind if kind == TextContent::CONTENT_TYPE => {
            TextBlock::body(&TextContent::decode(content)?)
                .map(|body| body.items())
                .unwrap_or_default()
        }
        kind if kind == ImageContent::CONTENT_TYPE => {
            ImageContent::decode(content)?.data().to_vec()
        }
        kind if kind == PdfContent::CONTENT_TYPE => PdfContent::decode(content)?.data().to_vec(),
        kind if kind == AudioContent::CONTENT_TYPE => {
            AudioContent::decode(content)?.data().to_vec()
        }
        kind if kind == FileContent::CONTENT_TYPE => FileContent::decode(content)?.data().to_vec(),
        _ => return Err(ContentError::Malformed("a block with no file form")),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileBlock {
    pub block_type: Uuid,
    pub content: Vec<u8>,
    pub references: Vec<Uuid>,
    pub derived: Entries,
}

impl FileBlock {
    fn of<C: BlockContent>(content: &C, organization: Uuid) -> Self {
        Self {
            block_type: C::CONTENT_TYPE,
            content: content.encode(),
            references: content.references_in(organization),
            derived: derived_metadata(content),
        }
    }
}

pub fn new_block(name: &str, bytes: Vec<u8>, organization: Uuid) -> FileBlock {
    let extension = extension(name);
    let media_type = media_type_of(name).to_owned();
    if ImageContent::FILE_EXTENSIONS.contains(&extension.as_str()) {
        let mut image = ImageContent::from_file(name, bytes);
        let mut header = image.header().clone();
        header.media_type = media_type;
        image.set_header(header);
        return FileBlock::of(&image, organization);
    }
    if extension == "pdf"
        && bytes.starts_with(b"%PDF-")
        && let Ok(pdf) = PdfContent::from_file(name, bytes.clone())
    {
        return FileBlock::of(&pdf, organization);
    }
    if media_type.starts_with("audio/")
        && !bytes.is_empty()
        && let Ok(audio) = AudioContent::from_file(name, media_type.clone(), bytes.clone())
    {
        return FileBlock::of(&audio, organization);
    }
    if is_text(&bytes) {
        let text = TextBlock::new(bytes).with_language(language_of(&extension));
        return FileBlock::of(&text.content(), organization);
    }
    FileBlock::of(
        &FileContent::from_file(name, media_type, bytes),
        organization,
    )
}

pub fn update_block(
    block_type: Uuid,
    current: Option<&[u8]>,
    name: &str,
    bytes: Vec<u8>,
    client: u64,
    organization: Uuid,
) -> Result<FileBlock, ContentError> {
    let media_type = media_type_of(name).to_owned();
    Ok(match block_type {
        kind if kind == TextContent::CONTENT_TYPE => {
            let mut text = match current {
                Some(current) => TextContent::decode(current)?,
                None => TextBlock::new("")
                    .with_language(language_of(&extension(name)))
                    .content(),
            };
            if let Some(edit) = TextBlock::rewrite(&text, client, &bytes) {
                text.apply(&edit);
            }
            FileBlock::of(&text, organization)
        }
        kind if kind == ImageContent::CONTENT_TYPE => {
            let mut image = ImageContent::from_file(name, bytes);
            let mut header = image.header().clone();
            header.media_type = media_type;
            image.set_header(header);
            FileBlock::of(&image, organization)
        }
        kind if kind == PdfContent::CONTENT_TYPE => FileBlock::of(
            &PdfContent::new(
                crate::PdfHeader {
                    source_name: name.to_owned(),
                },
                bytes,
            ),
            organization,
        ),
        kind if kind == AudioContent::CONTENT_TYPE => FileBlock::of(
            &AudioContent::new(
                crate::AudioHeader {
                    source_name: name.to_owned(),
                    media_type,
                },
                bytes,
            ),
            organization,
        ),
        kind if kind == FileContent::CONTENT_TYPE => FileBlock::of(
            &FileContent::from_file(name, media_type, bytes),
            organization,
        ),
        _ => return Err(ContentError::Malformed("a block with no file form")),
    })
}

pub fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

const MAX_FILE_NAME_BYTES: usize = 255;

pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("it has no name".to_owned());
    }
    if name == "." || name == ".." {
        return Some(format!("{name} names a directory itself"));
    }
    if let Some(character) = name
        .chars()
        .find(|character| FORBIDDEN.contains(character) || character.is_control())
    {
        return Some(match character {
            '/' | '\\' => format!("{character} separates directories in a path"),
            character if character.is_control() => {
                format!(
                    "it holds the control character U+{:04X}",
                    u32::from(character)
                )
            }
            character => format!("{character} is not allowed in a file name on Windows"),
        });
    }
    if name.ends_with(['.', ' ']) {
        return Some("Windows drops a dot or space at the end of a file name".to_owned());
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let upper = stem.to_ascii_uppercase();
    let numbered = ["COM", "LPT"].iter().any(|prefix| {
        upper.len() == 4 && upper.starts_with(prefix) && upper.as_bytes()[3].is_ascii_digit()
    });
    if RESERVED.contains(&upper.as_str()) || numbered {
        return Some(format!("{stem} is a device name on Windows"));
    }
    if name.len() > MAX_FILE_NAME_BYTES {
        return Some(format!(
            "it is longer than the {MAX_FILE_NAME_BYTES} bytes a file name may be"
        ));
    }
    None
}

pub fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => extension.to_ascii_lowercase(),
        _ => String::new(),
    }
}

pub fn language_of(extension: &str) -> TextLanguage {
    match extension {
        "md" | "markdown" => TextLanguage::Markdown,
        "rs" => TextLanguage::Rust,
        "zig" => TextLanguage::Zig,
        "toml" => TextLanguage::Toml,
        _ => TextLanguage::PlainText,
    }
}

pub fn text_file_type(language: TextLanguage) -> (&'static str, &'static str) {
    match language {
        TextLanguage::Markdown => ("md", "text/markdown"),
        TextLanguage::PlainText => ("txt", "text/plain"),
        TextLanguage::Rust => ("rs", "text/x-rust"),
        TextLanguage::Zig => ("zig", "text/x-zig"),
        TextLanguage::Toml => ("toml", "application/toml"),
    }
}

const MEDIA_TYPES: [(&str, &str); 17] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("tiff", "image/tiff"),
    ("tif", "image/tiff"),
    ("ico", "image/x-icon"),
    ("mp3", "audio/mpeg"),
    ("wav", "audio/wav"),
    ("ogg", "audio/ogg"),
    ("flac", "audio/flac"),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
    ("ttf", "font/ttf"),
    ("wasm", "application/wasm"),
];

pub fn media_type_of(name: &str) -> &'static str {
    let extension = extension(name);
    MEDIA_TYPES
        .iter()
        .find(|(known, _)| *known == extension)
        .map_or("application/octet-stream", |(_, media_type)| media_type)
}

pub fn extension_of(media_type: &str) -> &'static str {
    match media_type {
        "audio/x-wav" => "wav",
        media_type => MEDIA_TYPES
            .iter()
            .find(|(_, known)| *known == media_type)
            .map_or("", |(extension, _)| extension),
    }
}

pub fn export_name(name: &str, extension: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|character| match character {
            character if FORBIDDEN.contains(&character) => '-',
            character => character,
        })
        .collect();
    let stem = match stem.is_empty() {
        true => "Untitled".to_owned(),
        false => stem,
    };
    match extension.is_empty() || self::extension(&stem) == extension {
        true => stem,
        false => format!("{stem}.{extension}"),
    }
}
