use super::*;

#[test]
fn a_manifest_round_trips_through_the_vault() {
    let vault = vault(2);
    for length in [0, 1, 63, 64, 1024, 5000, 100_000] {
        let data = pseudorandom(length, length as u64 + 3);
        let manifest = vault.write(id(2), CONTENT, &data).unwrap();
        assert_eq!(manifest.length(), length as u64);
        assert_eq!(manifest.content_type, CONTENT);
        assert_eq!(manifest.is_empty(), length == 0);
        assert_eq!(vault.read(&manifest).unwrap(), data);
        assert!(
            vault
                .missing_objects(&manifest, 0, u64::MAX)
                .unwrap()
                .is_empty()
        );
        let stored = vault.load(&manifest).unwrap();
        assert_eq!(stored.bytes, data);
        assert!(stored.root.is_none_or(|root| root.children.is_empty()));
    }
}
