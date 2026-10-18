use std::{
    fs,
    io::{Read, Write},
};

use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[test]
fn metadata_preserves_alignment() {
    let directory = std::env::temp_dir().join(format!("apk-metadata-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let scratch = super::super::Scratch(directory);
    let apk = scratch.0.join("template.apk");
    let metadata = scratch.0.join("build-info.json");
    fs::write(&metadata, b"{\"id\":\"example\"}").unwrap();
    let mut writer = ZipWriter::new(fs::File::create(&apk).unwrap());
    writer
        .start_file(
            "lib/arm64-v8a/libexample.so",
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .with_alignment(16384),
        )
        .unwrap();
    writer.write_all(b"native library").unwrap();
    writer.finish().unwrap();
    let before = fs::read(&apk).unwrap();
    let offset = ZipArchive::new(fs::File::open(&apk).unwrap())
        .unwrap()
        .by_index(0)
        .unwrap()
        .data_start()
        .unwrap();
    assert_eq!(offset % 16384, 0);
    super::super::append_metadata(&apk, &metadata).unwrap();
    let after = fs::read(&apk).unwrap();
    assert_eq!(
        &before[..offset as usize + 14],
        &after[..offset as usize + 14]
    );
    let mut archive = ZipArchive::new(fs::File::open(&apk).unwrap()).unwrap();
    assert_eq!(archive.by_index(0).unwrap().data_start().unwrap(), offset);
    let mut contents = String::new();
    archive
        .by_name("assets/build-info.json")
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert_eq!(contents, "{\"id\":\"example\"}");
    assert!(super::super::append_metadata(&apk, &metadata).is_err());
}
