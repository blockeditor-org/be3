use be_block::plain::{extension, language_of, media_type_of};
use be_block::{
    AudioContent, AudioHeader, BlockContent, FileContent, ImageContent, PdfContent, PdfHeader,
    TextBlock, TextContent,
};

pub trait FileForm: BlockContent {
    fn to_file(&self) -> Option<Vec<u8>>;

    fn from_file(current: Option<&Self>, name: &str, bytes: Vec<u8>, client: u64) -> Option<Self>;

    fn claims(name: &str, first: &[u8]) -> bool;
}

pub(crate) fn to_file<C: FileForm>(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    Ok(C::decode(bytes)
        .map_err(|error| error.to_string())?
        .to_file())
}

pub(crate) fn from_file<C: FileForm>(
    current: Option<&[u8]>,
    name: &str,
    bytes: Vec<u8>,
    client: u64,
) -> Option<Vec<u8>> {
    let current = match current {
        Some(current) => Some(C::decode(current).ok()?),
        None => None,
    };
    Some(C::from_file(current.as_ref(), name, bytes, client)?.encode())
}

impl FileForm for TextContent {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(
            TextBlock::body(self)
                .map(|body| body.items())
                .unwrap_or_default(),
        )
    }

    fn from_file(current: Option<&Self>, name: &str, bytes: Vec<u8>, client: u64) -> Option<Self> {
        let mut text = match current {
            Some(current) => current.clone(),
            None => TextBlock::new("")
                .with_language(language_of(&extension(name)))
                .content(),
        };
        if let Some(edit) = TextBlock::rewrite(&text, client, &bytes) {
            text.apply(&edit);
        }
        Some(text)
    }

    fn claims(_: &str, first: &[u8]) -> bool {
        !first.contains(&0)
            && std::str::from_utf8(first).map_or_else(|error| error.error_len().is_none(), |_| true)
    }
}

impl FileForm for ImageContent {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(self.data().to_vec())
    }

    fn from_file(_: Option<&Self>, name: &str, bytes: Vec<u8>, _: u64) -> Option<Self> {
        let mut image = ImageContent::from_file(name, bytes);
        let mut header = image.header().clone();
        header.media_type = media_type_of(name).to_owned();
        image.set_header(header);
        Some(image)
    }

    fn claims(name: &str, _: &[u8]) -> bool {
        ImageContent::FILE_EXTENSIONS.contains(&extension(name).as_str())
    }
}

impl FileForm for PdfContent {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(self.data().to_vec())
    }

    fn from_file(_: Option<&Self>, name: &str, bytes: Vec<u8>, _: u64) -> Option<Self> {
        Some(PdfContent::new(
            PdfHeader {
                source_name: name.to_owned(),
            },
            bytes,
        ))
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        extension(name) == "pdf" && first.starts_with(b"%PDF-")
    }
}

impl FileForm for AudioContent {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(self.data().to_vec())
    }

    fn from_file(_: Option<&Self>, name: &str, bytes: Vec<u8>, _: u64) -> Option<Self> {
        Some(AudioContent::new(
            AudioHeader {
                source_name: name.to_owned(),
                media_type: media_type_of(name).to_owned(),
            },
            bytes,
        ))
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        media_type_of(name).starts_with("audio/") && !first.is_empty()
    }
}

impl FileForm for FileContent {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(self.data().to_vec())
    }

    fn from_file(_: Option<&Self>, name: &str, bytes: Vec<u8>, _: u64) -> Option<Self> {
        Some(FileContent::from_file(name, media_type_of(name), bytes))
    }

    fn claims(_: &str, _: &[u8]) -> bool {
        true
    }
}
