use super::*;

#[test]
fn put_content_addressed_asks_with_head() {
    let stub = Stub::start();
    let storage = Storage::new(Some(KEY.to_owned()));
    let sha256 = be_lib_sha256::sha256(b"be");
    let url = format!("{}/files/{sha256}", stub.url);

    let put = || {
        storage.put_content_addressed(&url, Body::Bytes(b"be"), &sha256, "application/octet-stream")
    };
    assert_eq!(put(), Ok(Put::Stored));
    assert_eq!(put(), Ok(Put::Present));
    assert_eq!(stub.puts(), 1, "the same bytes are not sent again");
    assert_eq!(stub.sent("GET"), 0, "nothing is downloaded to be hashed");
    assert_eq!(stub.sent("HEAD"), 2);
    let requests = stub.requests.lock().unwrap();
    let head = requests.iter().find(|request| request.method == "HEAD").unwrap();
    assert_eq!(head.headers["accesskey"], KEY, "the storage API is asked with the key");
    drop(requests);
    assert_eq!(stub.files.lock().unwrap()[&format!("/files/{sha256}")], b"be");
}
