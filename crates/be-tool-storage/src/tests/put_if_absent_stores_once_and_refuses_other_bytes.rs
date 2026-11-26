use super::*;

#[test]
fn put_if_absent_stores_once_and_refuses_other_bytes() {
    let stub = Stub::start();
    let storage = Storage::new(Some(KEY.to_owned()));
    let url = format!("{}/data/build/be", stub.url);
    let sha256 = be_lib_sha256::sha256(b"be");

    let put = |bytes: &[u8]| {
        storage.put_if_absent(
            &url,
            Body::Bytes(bytes),
            &be_lib_sha256::sha256(bytes),
            "application/octet-stream",
        )
    };
    assert_eq!(put(b"be"), Ok(Put::Stored));
    assert_eq!(put(b"be"), Ok(Put::Present));
    assert_eq!(stub.puts(), 1, "the same bytes are not sent again");
    let error = put(b"another be").unwrap_err();
    assert!(error.contains("refusing to overwrite"), "{error}");
    assert!(error.contains(&sha256), "{error}");
    assert_eq!(stub.puts(), 1);

    let mut stored = Vec::new();
    storage
        .get(&url)
        .unwrap()
        .expect("the zone holds it")
        .read_to_end(&mut stored)
        .unwrap();
    assert_eq!(stored, b"be");
    assert_eq!(stub.files.lock().unwrap()["/data/build/be"], b"be");
}
