use super::*;

#[test]
fn a_file_store_persists_objects_across_reopening() {
    let root = std::env::temp_dir().join(format!("be-store-test-{}", Uuid::new_v4()));
    let data = pseudorandom(40_000, 41);
    let manifest = {
        let vault =
            Vault::new(FileStore::open(&root).unwrap(), key(8)).with_chunker(ChunkerConfig::SMALL);
        vault.write(CONTENT, &data).unwrap()
    };

    let reopened =
        Vault::new(FileStore::open(&root).unwrap(), key(8)).with_chunker(ChunkerConfig::SMALL);
    assert_eq!(reopened.read(&manifest).unwrap(), data);
    assert!(reopened.missing_chunks(&manifest).unwrap().is_empty());

    let dropped = manifest.chunks[0].hash;
    reopened.store().remove(dropped).unwrap();
    assert_eq!(reopened.missing_chunks(&manifest).unwrap(), vec![dropped]);

    std::fs::remove_dir_all(&root).unwrap();
}
