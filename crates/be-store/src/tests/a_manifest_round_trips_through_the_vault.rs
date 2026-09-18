use super::*;

#[test]
fn a_manifest_round_trips_through_the_vault() {
    let vault = vault(2);
    for length in [0, 1, 63, 64, 1024, 5000, 100_000] {
        let data = pseudorandom(length, length as u64 + 3);
        let manifest = vault.write(CONTENT, &data).unwrap();
        assert_eq!(manifest.length, length as u64);
        assert_eq!(manifest.content_type, CONTENT);
        assert_eq!(
            manifest
                .chunks
                .iter()
                .map(|chunk| chunk.length)
                .sum::<u32>() as usize,
            length
        );
        assert_eq!(vault.read(&manifest).unwrap(), data);
        assert!(vault.missing_chunks(&manifest).unwrap().is_empty());
    }
}
