use super::*;

#[test]
fn identical_content_deduplicates_across_writes() {
    let vault = vault(1);
    let data = pseudorandom(32 * 1024, 11);
    let first = vault.write(CONTENT, &data).unwrap();
    let objects = vault.store().count();
    let second = vault.write(CONTENT, &data).unwrap();
    assert_eq!(first, second);
    assert_eq!(vault.store().count(), objects);

    let mut appended = data.clone();
    appended.extend_from_slice(&pseudorandom(4 * 1024, 12));
    let third = vault.write(CONTENT, &appended).unwrap();
    let shared = third
        .chunks
        .iter()
        .filter(|chunk| first.chunks.contains(chunk))
        .count();
    assert!(
        shared * 2 > first.chunks.len(),
        "appending re-uploaded all but {shared} chunks"
    );
}
