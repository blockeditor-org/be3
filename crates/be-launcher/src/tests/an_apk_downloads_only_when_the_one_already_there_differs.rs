use super::*;

#[test]
fn an_apk_downloads_only_when_the_one_already_there_differs() {
    let directory = scratch("changed");
    let apk = directory.join("app.apk");
    let slot = Slot::PullRequest(7);
    let mut store = Store::default();
    let first = store.put(slot, b"app one");

    let mut reported = Vec::new();
    builds::fetch_apk(&apk, slot, &first, &mut store, &mut |done, total| {
        reported.push((done, total));
    })
    .unwrap();
    assert_eq!(store.fetched, vec![slot.object_url(&first.hash)]);
    assert_eq!(reported.last(), Some(&(first.size, first.size)));
    assert_eq!(std::fs::read(&apk).unwrap(), b"app one");

    store.fetched.clear();
    fetch_apk(&apk, slot, &first, &mut store).unwrap();
    assert!(store.fetched.is_empty());

    let second = store.put(slot, b"app two");
    fetch_apk(&apk, slot, &second, &mut store).unwrap();
    assert_eq!(store.fetched, vec![slot.object_url(&second.hash)]);
    assert_eq!(std::fs::read(&apk).unwrap(), b"app two");

    let record = directory.join("installed.json");
    let installed = builds::Installed {
        slot,
        commit: "second".to_owned(),
        hash: second.hash,
    };
    builds::remember(&record, Some(&installed)).unwrap();
    assert_eq!(builds::installed(&record), Some(installed));
    builds::remember(&record, None).unwrap();
    assert_eq!(builds::installed(&record), None);
    let _ = std::fs::remove_dir_all(&directory);
}
