use super::*;

/// Each extension a file picker filters by has a media type of its own
/// (`plain::media_type_of`), so a file picked by it is stored as what it is.
#[test]
fn every_extension_it_lists_has_a_media_type() {
    for extension in PdfFile::FILE_EXTENSIONS {
        let media_type = be_block_content::plain::media_type_of(&format!("file.{extension}"));
        assert_ne!(media_type, "application/octet-stream", "{extension} has no media type");
    }
}
