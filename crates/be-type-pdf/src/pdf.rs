use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block_content::blob::{Blob, BlobKind};
use be_block_content::plain::extension;
use be_sdk_type_guest::BlobFile;

const PDF_MAGIC: &[u8] = b"%PDF-";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PdfHeader {
    pub source_name: String,
}

pub struct PdfFile;

impl BlobKind for PdfFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7064_662d_626c_6f63_6b2d_7479_7065_2d01);

    type Header = PdfHeader;

    fn name(header: &PdfHeader) -> &str {
        &header.source_name
    }
}

pub type PdfContent = Blob<PdfFile>;

impl PdfFile {
    /// Whether `data` is a PDF file, whatever its name.
    pub fn is_pdf(data: &[u8]) -> bool {
        data.starts_with(PDF_MAGIC)
    }
}

impl BlobFile for PdfFile {
    const FILE_EXTENSIONS: &'static [&'static str] = &["pdf"];
    const MIME_TYPES: &'static [&'static str] = &["application/pdf"];
    fn header(name: &str, _: &[u8]) -> PdfHeader {
        PdfHeader {
            source_name: name.to_owned(),
        }
    }

    fn media_type(_: &PdfHeader) -> String {
        "application/pdf".to_owned()
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        Self::FILE_EXTENSIONS.contains(&extension(name).as_str()) && Self::is_pdf(first)
    }
}
