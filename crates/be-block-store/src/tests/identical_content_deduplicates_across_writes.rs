use std::collections::HashSet;

use super::*;

#[test]
fn identical_content_deduplicates_across_writes() {
    let vault = vault(1);
    let data = pseudorandom(32 * 1024, 11);
    let first = vault.write(id(1), CONTENT, &data).unwrap();
    let objects = vault.store().count();
    let second = vault.write(id(1), CONTENT, &data).unwrap();
    assert_eq!(first, second);
    assert_eq!(vault.store().count(), objects);

    let mut appended = data.clone();
    appended.extend_from_slice(&pseudorandom(4 * 1024, 12));
    let third = vault.write(id(1), CONTENT, &appended).unwrap();
    let before: HashSet<_> = vault.objects(&first).unwrap().into_iter().collect();
    let after = vault.objects(&third).unwrap();
    let shared = after.iter().filter(|hash| before.contains(hash)).count();
    assert!(
        shared * 2 > before.len(),
        "appending re-uploaded all but {shared} objects"
    );
    let new = vault.objects_besides(&third, &before).unwrap();
    assert!(
        new.len() * 4 < after.len(),
        "appending wrote {} new objects of {}",
        new.len(),
        after.len()
    );
}
