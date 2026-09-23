use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::blob::{Blob, BlobKind};

const PDF_MAGIC: &[u8] = b"%PDF-";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PdfHeader {
    pub source_name: String,
}

pub struct PdfFile;

impl BlobKind for PdfFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7064_662d_626c_6f63_6b2d_7479_7065_0002);

    type Header = PdfHeader;

    fn name(header: &PdfHeader) -> &str {
        &header.source_name
    }
}

pub type PdfContent = Blob<PdfFile>;

impl Blob<PdfFile> {
    pub fn from_file(source_name: impl Into<String>, data: Vec<u8>) -> Result<Self, String> {
        if !data.starts_with(PDF_MAGIC) {
            return Err("data is not a PDF file".into());
        }
        Ok(Self::new(
            PdfHeader {
                source_name: source_name.into(),
            },
            data,
        ))
    }
}
