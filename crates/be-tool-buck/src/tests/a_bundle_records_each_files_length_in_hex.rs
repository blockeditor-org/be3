use crate::bundle::record;

#[test]
fn a_bundle_records_each_files_length_in_hex() {
    let directory = std::env::temp_dir().join(format!("bundle-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let manifest = directory.join("manifest.json");
    std::fs::write(&manifest, vec![b'x'; 26]).unwrap();
    let mut bundle = Vec::new();
    record(&mut bundle, &manifest.to_string_lossy()).unwrap();
    record(&mut bundle, "").unwrap();
    assert_eq!(
        bundle,
        [b"0000001a".as_slice(), &[b'x'; 26], b"00000000"].concat()
    );
    let _ = std::fs::remove_dir_all(directory);
}
