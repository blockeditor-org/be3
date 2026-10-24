use be_block::{
    AudioContent, BlockContent, FileContent, ImageContent, PdfContent, TextContent, plain,
};
use uuid::Uuid;

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

fn plain_file<C: BlockContent>(content: &C) -> Option<Vec<u8>> {
    plain::file_bytes(C::CONTENT_TYPE, &content.encode()).ok()
}

fn plain_block<C: BlockContent>(
    current: Option<&C>,
    name: &str,
    bytes: Vec<u8>,
    client: u64,
) -> Option<C> {
    let current = current.map(BlockContent::encode);
    let block = plain::update_block(
        C::CONTENT_TYPE,
        current.as_deref(),
        name,
        bytes,
        client,
        Uuid::nil(),
    )
    .ok()?;
    C::decode(&block.content).ok()
}

macro_rules! plain_form {
    ($kind:ty, $claims:expr) => {
        impl FileForm for $kind {
            fn to_file(&self) -> Option<Vec<u8>> {
                plain_file(self)
            }

            fn from_file(
                current: Option<&Self>,
                name: &str,
                bytes: Vec<u8>,
                client: u64,
            ) -> Option<Self> {
                plain_block(current, name, bytes, client)
            }

            fn claims(name: &str, first: &[u8]) -> bool {
                let claims: fn(&str, &[u8]) -> bool = $claims;
                claims(name, first)
            }
        }
    };
}

plain_form!(TextContent, |_, first| {
    !first.contains(&0)
        && std::str::from_utf8(first).map_or_else(|error| error.error_len().is_none(), |_| true)
});
plain_form!(ImageContent, |name, _| {
    ImageContent::FILE_EXTENSIONS.contains(&plain::extension(name).as_str())
});
plain_form!(PdfContent, |name, first| {
    plain::extension(name) == "pdf" && first.starts_with(b"%PDF-")
});
plain_form!(AudioContent, |name, first| {
    plain::media_type_of(name).starts_with("audio/") && !first.is_empty()
});
plain_form!(FileContent, |_, _| true);
