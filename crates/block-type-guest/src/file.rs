use be_block::be_model::Document;
use be_block::blob::{Blob, BlobKind};
use be_block::file::{AnyFile, FileHeader};
use be_block::plain::{extension, language_of, media_type_of};
use be_block::{BlockContent, Root, TextBlock};

pub trait FileForm: BlockContent {
    fn to_file(&self) -> Option<Vec<u8>>;

    fn from_file(current: Option<&Self>, name: &str, bytes: Vec<u8>, client: u64) -> Option<Self>;

    fn claims(name: &str, first: &[u8]) -> bool;
}

pub trait BlobFile: BlobKind {
    fn header(name: &str, bytes: &[u8]) -> Self::Header;

    fn claims(name: &str, first: &[u8]) -> bool;
}

pub trait DocumentFile: Root {
    fn to_file(content: &Document<Self>) -> Vec<u8>;

    fn from_file(
        current: Option<&Document<Self>>,
        name: &str,
        bytes: Vec<u8>,
        client: u64,
    ) -> Option<Document<Self>>;

    fn claims(name: &str, first: &[u8]) -> bool;
}

impl<K: BlobFile> FileForm for Blob<K> {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(self.data().to_vec())
    }

    fn from_file(_: Option<&Self>, name: &str, bytes: Vec<u8>, _: u64) -> Option<Self> {
        Some(Self::new(K::header(name, &bytes), bytes))
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        K::claims(name, first)
    }
}

impl<R: DocumentFile> FileForm for Document<R> {
    fn to_file(&self) -> Option<Vec<u8>> {
        Some(R::to_file(self))
    }

    fn from_file(current: Option<&Self>, name: &str, bytes: Vec<u8>, client: u64) -> Option<Self> {
        R::from_file(current, name, bytes, client)
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        R::claims(name, first)
    }
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

impl BlobFile for AnyFile {
    fn header(name: &str, _: &[u8]) -> FileHeader {
        FileHeader {
            source_name: name.to_owned(),
            media_type: media_type_of(name).to_owned(),
        }
    }

    fn claims(_: &str, _: &[u8]) -> bool {
        true
    }
}

impl DocumentFile for TextBlock {
    fn to_file(content: &Document<Self>) -> Vec<u8> {
        TextBlock::body(content)
            .map(|body| body.items())
            .unwrap_or_default()
    }

    fn from_file(
        current: Option<&Document<Self>>,
        name: &str,
        bytes: Vec<u8>,
        client: u64,
    ) -> Option<Document<Self>> {
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
