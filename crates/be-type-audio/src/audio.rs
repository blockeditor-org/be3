use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block_content::blob::{Blob, BlobKind};
use be_block_content::plain::{extension, media_type_of};
use be_sdk_type_guest::BlobFile;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioHeader {
    pub source_name: String,
    pub media_type: String,
}

pub struct AudioFile;

impl BlobKind for AudioFile {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6175_6469_6f2d_626c_6f63_6b2d_7479_7001);

    type Header = AudioHeader;

    fn name(header: &AudioHeader) -> &str {
        &header.source_name
    }
}

pub type AudioContent = Blob<AudioFile>;

impl BlobFile for AudioFile {
    const FILE_EXTENSIONS: &'static [&'static str] = &["mp3", "wav", "ogg", "oga", "flac", "m4a"];
    const MIME_TYPES: &'static [&'static str] = &["audio/*"];

    /// Its media type by its extension; one with none of its extensions is taken as MP3, the
    /// commonest.
    fn header(name: &str, _: &[u8]) -> AudioHeader {
        let media_type = match Self::FILE_EXTENSIONS.contains(&extension(name).as_str()) {
            true => media_type_of(name),
            false => "audio/mpeg",
        };
        AudioHeader {
            source_name: name.to_owned(),
            media_type: media_type.to_owned(),
        }
    }

    fn media_type(header: &AudioHeader) -> String {
        header.media_type.clone()
    }

    fn claims(name: &str, first: &[u8]) -> bool {
        Self::FILE_EXTENSIONS.contains(&extension(name).as_str()) && !first.is_empty()
    }
}
