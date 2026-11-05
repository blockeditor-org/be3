use super::*;

#[test]
fn the_cache_in_front_of_a_bucket_keeps_only_its_limit() {
    let server = TestBucket::start();
    let data = std::env::temp_dir().join(format!("be-server-cache-{}", Uuid::new_v4()));
    let objects = Objects::open(&data, Some(&server.config(100))).unwrap();
    let Objects::Bucket(cached) = &objects else {
        panic!("a configured bucket is used");
    };
    let object = |byte: u8| vec![byte; 40];
    let first = objects.put(&object(1)).unwrap();
    let second = objects.put(&object(2)).unwrap();
    assert_eq!(cached.cached_bytes(), 80);
    // Reading the first makes the second the least recently used.
    assert_eq!(objects.get(first).unwrap(), Some(object(1)));
    let third = objects.put(&object(3)).unwrap();
    assert_eq!(cached.cached_bytes(), 80);
    let cache = FileStore::open(data.join("object-cache")).unwrap();
    assert!(cache.has(first).unwrap());
    assert!(!cache.has(second).unwrap());
    assert!(cache.has(third).unwrap());

    // Every object is still in the bucket, and one read again is cached again.
    assert_eq!(objects.get(second).unwrap(), Some(object(2)));
    assert!(cache.has(second).unwrap());
    assert_eq!(cached.cached_bytes(), 80);
    let read = objects.get_many(&[first, second, third, Hash::of(b"absent")]);
    let read: Vec<_> = read.into_iter().map(Result::unwrap).collect();
    assert_eq!(read, vec![Some(object(1)), Some(object(2)), Some(object(3)), None]);

    // An object larger than the whole cache goes only to the bucket.
    let large = objects.put(&[9; 200]).unwrap();
    assert!(!cache.has(large).unwrap());
    assert_eq!(objects.get(large).unwrap(), Some(vec![9; 200]));

    // Reopened, the cache keeps what it held, within the limit.
    drop(objects);
    let reopened = Objects::open(&data, Some(&server.config(50))).unwrap();
    let Objects::Bucket(cached) = &reopened else {
        panic!("a configured bucket is used");
    };
    assert_eq!(cached.cached_bytes(), 40);
    objects_removed_from_both(&reopened, &cache, second);
    std::fs::remove_dir_all(data).unwrap();
}

fn objects_removed_from_both(objects: &Objects, cache: &FileStore, hash: Hash) {
    objects.remove(hash).unwrap();
    assert!(!cache.has(hash).unwrap());
    assert!(!objects.has(hash).unwrap());
    assert_eq!(objects.get(hash).unwrap(), None);
}
