use super::*;

#[test]
fn a_bucket_store_round_trips_objects() {
    let server = TestBucket::start();
    let store = S3Store::new(server.bucket());
    let first = store.put(b"first object").unwrap();
    let second = store.put(b"second object, a little longer").unwrap();
    assert_eq!(first, Hash::of(b"first object"));
    assert_eq!(
        store.get(first).unwrap().as_deref(),
        Some(&b"first object"[..])
    );
    assert!(store.has(second).unwrap());
    assert_eq!(store.len(second).unwrap(), Some(30));

    let absent = Hash::of(b"never stored");
    assert_eq!(store.get(absent).unwrap(), None);
    assert!(!store.has(absent).unwrap());
    assert_eq!(store.len(absent).unwrap(), None);

    let mut listed = store.hashes().unwrap();
    listed.sort_unstable();
    let mut expected = vec![first, second];
    expected.sort_unstable();
    assert_eq!(listed, expected);

    store.remove(first).unwrap();
    store.remove(first).unwrap();
    assert_eq!(store.get(first).unwrap(), None);
    assert_eq!(store.hashes().unwrap(), vec![second]);

    let url = server.bucket().presign_get(second, Duration::from_secs(60));
    let mut read = Vec::new();
    std::io::Read::read_to_end(
        &mut agent().get(&url).call().unwrap().into_reader(),
        &mut read,
    )
    .unwrap();
    assert_eq!(read, b"second object, a little longer");
}
