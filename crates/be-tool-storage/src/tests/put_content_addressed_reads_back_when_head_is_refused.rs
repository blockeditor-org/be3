use super::*;

#[test]
fn put_content_addressed_reads_back_when_head_is_refused() {
    let stub = Stub::refusing_keyed_head();
    let storage = Storage::new(Some(KEY.to_owned()));
    let sha256 = be_lib_sha256::sha256(b"be");
    let url = format!("{}/files/{sha256}", stub.url);

    let put = |bytes: &[u8]| {
        storage.put_content_addressed(
            &url,
            Body::Bytes(bytes),
            &be_lib_sha256::sha256(bytes),
            "application/octet-stream",
        )
    };
    assert_eq!(put(b"be"), Ok(Put::Stored));
    assert_eq!(put(b"be"), Ok(Put::Present));
    assert_eq!(stub.puts(), 1, "the same bytes are not sent again");
    assert_eq!(stub.sent("GET"), 2, "each refused HEAD is followed by reading the file back");

    // What the fallback still catches, which a HEAD could not: other bytes at the path.
    let error = put(b"another be").unwrap_err();
    assert!(error.contains("refusing to overwrite"), "{error}");
    assert_eq!(stub.puts(), 1);
}
