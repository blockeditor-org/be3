use be_block::be_model::Document;
use be_block::blob::{Blob, BlobKind};
use be_block::{BlockContent, Root};

pub trait FileForm: BlockContent {
    const ANY_FILE: bool = false;

    fn to_file(&self) -> Option<Vec<u8>>;

    fn from_file(current: Option<&Self>, name: &str, bytes: Vec<u8>, client: u64) -> Option<Self>;

    fn claims(name: &str, first: &[u8]) -> bool;
}

pub trait BlobFile: BlobKind {
    const ANY_FILE: bool = false;

    fn header(name: &str, bytes: &[u8]) -> Self::Header;

    fn claims(name: &str, first: &[u8]) -> bool;
}

pub trait DocumentFile: Root {
    const ANY_FILE: bool = false;

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
    const ANY_FILE: bool = K::ANY_FILE;

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
    const ANY_FILE: bool = R::ANY_FILE;

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
