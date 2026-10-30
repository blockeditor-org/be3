use super::*;

#[test]
fn an_archive_naming_a_file_outside_its_directory_is_refused() {
    let scratch = std::env::temp_dir().join(format!("app-archive-escape-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).unwrap();
    let archive = scratch.join("escape.appz");
    let mut out = File::create(&archive).unwrap();
    out.write_all(MAGIC).unwrap();
    let mut encoder = zstd::Encoder::new(out, LEVEL).unwrap();
    let name = b"../escaped";
    encoder
        .write_all(&(name.len() as u32).to_le_bytes())
        .unwrap();
    encoder.write_all(name).unwrap();
    encoder.write_all(&[0]).unwrap();
    encoder.write_all(&1u64.to_le_bytes()).unwrap();
    encoder.write_all(b"x").unwrap();
    encoder.write_all(&0u32.to_le_bytes()).unwrap();
    encoder.finish().unwrap();
    let error = unpack(&archive, &scratch.join("out")).unwrap_err();
    assert!(error.contains("../escaped"), "{error}");
    assert!(!scratch.join("escaped").exists());
    fs::remove_dir_all(&scratch).unwrap();
}
