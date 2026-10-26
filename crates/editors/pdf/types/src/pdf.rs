use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::blob::{Blob, BlobKind};
use be_block::plain::extension;
use block_type_guest::BlobFile;

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
    pub fn from_file(source_name: impl Into<String>, data: Vec<u8>) -> Result<PdfContent, String> {
        if !data.starts_with(PDF_MAGIC) {
            return Err("data is not a PDF file".into());
        }
        Ok(PdfContent::new(
            PdfHeader {
                source_name: source_name.into(),
            },
            data,
        ))
    }
}

impl BlobFile for PdfFile {
    fn header(name: &str, _: &[u8]) -> PdfHeader {
        PdfHeader {
            source_name: name.to_owned(),
        }
    }

    fn media_type(_: &PdfHeader) -> String {
        "application/pdf".to_owned()
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        extension(name) == "pdf" && first.starts_with(PDF_MAGIC)
    }
}
