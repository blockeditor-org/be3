use super::*;

#[test]
fn a_tampered_object_reads_as_missing() {
    let server = TestBucket::start();
    let store = S3Store::new(server.bucket());
    let hash = store.put(b"the bytes that were stored").unwrap();
    std::fs::write(server.file(hash), b"the bytes that came back").unwrap();
    assert_eq!(store.get(hash).unwrap(), None);

    let data = std::env::temp_dir().join(format!("be-server-cache-{}", Uuid::new_v4()));
    let objects = Objects::open(&data, Some(&server.config(1 << 20))).unwrap();
    assert_eq!(objects.get(hash).unwrap(), None);
    objects.put(b"the bytes that were stored").unwrap();
    assert_eq!(
        objects.get(hash).unwrap().as_deref(),
        Some(&b"the bytes that were stored"[..])
    );
    std::fs::remove_dir_all(data).unwrap();
}
