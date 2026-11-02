use std::io::{Cursor, Read, Write};

use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[test]
fn metadata_preserves_alignment() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "lib/arm64-v8a/libexample.so",
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .with_alignment(16384),
        )
        .unwrap();
    writer.write_all(b"native library").unwrap();
    let before = writer.finish().unwrap().into_inner();
    let offset = ZipArchive::new(Cursor::new(&before))
        .unwrap()
        .by_index(0)
        .unwrap()
        .data_start()
        .unwrap();
    assert_eq!(offset % 16384, 0);
    let after = super::super::append_metadata(before.clone(), b"{\"id\":\"example\"}").unwrap();
    assert_eq!(
        &before[..offset as usize + 14],
        &after[..offset as usize + 14]
    );
    let mut archive = ZipArchive::new(Cursor::new(&after)).unwrap();
    assert_eq!(archive.by_index(0).unwrap().data_start().unwrap(), offset);
    let mut contents = String::new();
    archive
        .by_name("assets/build-info.json")
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert_eq!(contents, "{\"id\":\"example\"}");
    assert!(super::super::append_metadata(after, b"{}").is_err());
}
