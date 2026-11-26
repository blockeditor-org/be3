use super::*;

#[test]
fn nothing_is_sent_without_a_key() {
    let stub = Stub::start();
    let storage = Storage::new(None);
    let url = format!("{}/builds/android/commit.json", stub.url);
    let error = storage
        .put(
            &url,
            Body::Bytes(b"{}"),
            &be_lib_sha256::sha256(b"{}"),
            "application/json",
        )
        .unwrap_err();
    assert!(error.contains("no storage key"), "{error}");
    assert!(storage.get(&url).is_err());
    assert_eq!(storage.head(&url), Ok(false), "HEAD needs no key");
    assert_eq!(stub.requests.lock().unwrap().len(), 1);
}
