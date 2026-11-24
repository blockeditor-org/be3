use super::*;

#[test]
fn a_file_goes_whole_with_its_length_and_checksum() {
    let stub = Stub::start();
    let storage = Storage::new(Some(KEY.to_owned()));
    let file = std::env::temp_dir().join(format!("be-tool-storage-{}", std::process::id()));
    let bytes = vec![7u8; 100_000];
    std::fs::write(&file, &bytes).unwrap();
    let (sha256, _) = be_lib_sha256::sha256_read(File::open(&file).unwrap()).unwrap();
    let url = format!("{}/files/{sha256}", stub.url);

    assert_eq!(storage.head(&url), Ok(false));
    assert!(storage.get(&url).unwrap().is_none());
    storage
        .put(&url, Body::File(&file), &sha256, "application/octet-stream")
        .unwrap();
    std::fs::remove_file(&file).unwrap();
    assert_eq!(storage.head(&url), Ok(true));

    let requests = stub.requests.lock().unwrap();
    let put = requests.iter().find(|request| request.method == "PUT").unwrap();
    assert_eq!(put.path, format!("/files/{sha256}"));
    assert_eq!(put.headers["content-length"], "100000");
    assert_eq!(put.headers["checksum"], sha256.to_uppercase());
    assert_eq!(put.headers["content-type"], "application/octet-stream");
    assert!(!put.headers.contains_key("transfer-encoding"));
    let head = requests.iter().find(|request| request.method == "HEAD").unwrap();
    assert!(!head.headers.contains_key("accesskey"), "HEAD goes to the CDN without the key");
    drop(requests);
    assert_eq!(stub.files.lock().unwrap()[&format!("/files/{sha256}")], bytes);
}
